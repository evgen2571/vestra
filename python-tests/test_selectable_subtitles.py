import json
from pathlib import Path
import subprocess
import wave

import pytest

import vestra


def probe(path: Path, *args: str) -> dict:
    return json.loads(
        subprocess.check_output(
            ["ffprobe", "-v", "error", *args, "-of", "json", str(path)]
        )
    )


def extract_cue_text(path: Path) -> list[str]:
    data = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-map",
            "0:s:0",
            "-c",
            "copy",
            "-f",
            "data",
            "-",
        ]
    )
    text = []
    cursor = 0
    while cursor < len(data):
        size = int.from_bytes(data[cursor : cursor + 2], "big")
        cursor += 2
        if size:
            text.append(data[cursor : cursor + size].decode("utf-8"))
        cursor += size
    assert cursor == len(data)
    return text


def with_audio(tmp_path: Path) -> vestra.Project:
    media = tmp_path / "tone.wav"
    with wave.open(str(media), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(48_000)
        writer.writeframes(b"\x00\x18" * 48_000)
    project = vestra.Project(
        size=(16, 16),
        fps=(30_000, 1001),
        duration=1,
        output_audio=True,
    )
    project.audio.track("tone").add(media, trim_end=1)
    return project


@pytest.mark.parametrize("prepared", [False, True])
def test_published_mp4_contains_rebased_selectable_cues(tmp_path: Path, prepared: bool):
    project = with_audio(tmp_path)
    published = tmp_path / "selectable.mp4"
    baseline = tmp_path / "baseline.mp4"
    # Rounded to frame boundaries at 0.2002 and 0.4004, respectively.
    start, end = 0.18, 0.39
    first = "  <i>literal</i> & café 🙂\n\n\\N"
    last = "last cue"
    cues = [(0.35, 0.6, last), (0.1, 0.25, first), (0.0, 0.1, "outside")]

    if prepared:
        ready = project.prepare(backend="cpu")
        for output, subtitles in [(published, cues), (baseline, None)]:
            result = ready.render_video(
                vestra.PreparedVideoRenderRequest(
                    output,
                    start=start,
                    end=end,
                    subtitles=subtitles,
                ),
                show_progress=False,
            )
    else:
        for output, subtitles in [(published, cues), (baseline, None)]:
            result = project.render(
                output,
                start=start,
                end=end,
                subtitles=subtitles,
                backend="cpu",
                show_progress=False,
            )
    assert result.audio_present and result.total_frames == 6

    streams = probe(published, "-show_streams")["streams"]
    assert [(stream["codec_type"], stream["codec_name"]) for stream in streams] == [
        ("video", "h264"),
        ("audio", "aac"),
        ("subtitle", "mov_text"),
    ]
    assert streams[2]["disposition"]["forced"] == 0
    assert streams[2]["extradata_size"] >= 43
    assert int(streams[0]["nb_frames"]) == 6
    assert float(streams[0]["duration"]) == pytest.approx(0.2002, abs=0.001)
    assert float(streams[1]["duration"]) == pytest.approx(0.2002, abs=0.001)

    baseline_streams = probe(baseline, "-show_streams")["streams"]
    assert baseline_streams == streams[:2]
    for specifier in ["v", "a"]:
        fields = ("pts", "dts", "duration", "data_hash")
        args = (
            "-select_streams",
            specifier,
            "-show_packets",
            "-show_data_hash",
            "sha256",
        )
        with_subtitles = probe(published, *args)["packets"]
        without_subtitles = probe(baseline, *args)["packets"]
        assert [
            tuple(packet.get(key) for key in fields) for packet in with_subtitles
        ] == [tuple(packet.get(key) for key in fields) for packet in without_subtitles]

    packets = probe(published, "-select_streams", "s", "-show_packets")["packets"]
    cues_timing = [
        (float(packet["pts_time"]), float(packet["duration_time"]))
        for packet in packets
        if int(packet["size"]) > 2
    ]
    assert cues_timing == [(0.0, 0.05), (0.15, 0.05)]
    assert extract_cue_text(published) == [first, last]
    assert not list(tmp_path.glob(".vestra-*"))


def test_static_mp4_keeps_late_subtitle(tmp_path: Path):
    path = tmp_path / "static.mp4"
    project = with_audio(tmp_path)
    result = project.render(
        path,
        subtitles=[(0.9, 1.1, "end")],
        backend="cpu",
        show_progress=False,
    )
    assert result.audio_present and result.total_frames == 30
    packets = probe(path, "-select_streams", "s", "-show_packets")["packets"]
    assert [
        (float(packet["pts_time"]), float(packet["duration_time"]))
        for packet in packets
        if int(packet["size"]) > 2
    ] == [(0.9, 0.1)]
    assert extract_cue_text(path) == ["end"]


@pytest.mark.parametrize(
    "extension,cues,message",
    [
        ("mp4", [(0.4, 0.3, "reversed")], "end must be greater"),
        ("mp4", [(0.1, 0.4, "first"), (0.3, 0.5, "overlap")], "must not overlap"),
        ("mp4", [(0, 0.5, "bad\0text")], "contain no NUL"),
        ("gif", [(0, 0.5, "cue")], "require MP4"),
    ],
)
def test_invalid_subtitles_preserve_existing_destination(
    tmp_path: Path,
    extension: str,
    cues: list[tuple[float, float, str]],
    message: str,
):
    path = tmp_path / f"preserve.{extension}"
    path.write_bytes(b"original")
    project = vestra.Project(size=(16, 16), fps=10, duration=1)
    with pytest.raises(vestra.RenderError, match=message) as caught:
        project.render(
            path,
            subtitles=cues,
            backend="cpu",
            overwrite=True,
            show_progress=False,
        )
    assert caught.value.diagnostics[0].code == "VESTRA-RENDER-SUBTITLES"
    assert path.read_bytes() == b"original"
    assert list(tmp_path.iterdir()) == [path]


@pytest.mark.parametrize(
    "factory", [vestra.RenderRequest, vestra.PreparedVideoRenderRequest]
)
def test_subtitle_requests_validate_python_types(factory):
    for invalid in [True, "0.3"]:
        with pytest.raises(TypeError, match="subtitle cue 1"):
            factory("out.mp4", subtitles=[(invalid, 0.5, "cue")])


def test_subtitle_cancellation_cleans_staged_track(tmp_path: Path):
    path = tmp_path / "cancelled.mp4"
    path.write_bytes(b"original")
    cancellation = vestra.CancellationToken()

    def cancel_after_write(event: vestra.RenderEvent) -> None:
        if event.kind == "progress":
            cancellation.cancel()

    project = vestra.Project(size=(16, 16), fps=10, duration=1)
    with pytest.raises(vestra.CancelledError):
        project.render(
            path,
            subtitles=[(0, 0.5, "cue")],
            start=0.1,
            end=0.6,
            backend="cpu",
            overwrite=True,
            cancellation=cancellation,
            on_progress=cancel_after_write,
        )
    assert path.read_bytes() == b"original"
    assert list(tmp_path.iterdir()) == [path]
