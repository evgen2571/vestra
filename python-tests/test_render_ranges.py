import json
from pathlib import Path
import subprocess
import wave

import numpy as np
import pytest

import vestra
from vestra.authoring import Quality


def project() -> vestra.Project:
    value = vestra.Project(
        size=(16, 16), fps=10, duration=1, background="#102030", quality=Quality.HIGH
    )
    layer = value.root.add(vestra.sources.Color("#ff8040"))
    layer.opacity.keyframe(0, 0)
    layer.opacity.keyframe(1, 1)
    value.root.add(vestra.Rectangle(width=8, height=8, fill="#20ff40"), start=0.4)
    return value


def probe(path: Path) -> dict:
    return json.loads(
        subprocess.check_output(
            [
                "ffprobe",
                "-v",
                "error",
                "-show_streams",
                "-show_frames",
                "-of",
                "json",
                str(path),
            ]
        )
    )


def pixels(path: Path) -> np.ndarray:
    raw = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]
    )
    return np.frombuffer(raw, dtype=np.uint8).reshape(-1, 16, 16, 3)


@pytest.mark.parametrize("prepared", [False, True])
def test_video_range_matches_full_timeline_and_rebases_timestamps(
    tmp_path: Path, prepared: bool
):
    value = project()
    before = value.snapshot().to_dict()
    full = tmp_path / "full.mp4"
    clip = tmp_path / "clip.mp4"
    value.render(full, backend="cpu", show_progress=False)
    events: list[vestra.RenderEvent] = []
    if prepared:
        ready = value.prepare(backend="cpu")
        result = ready.render_video(
            vestra.PreparedVideoRenderRequest(clip, start=0.25, end=0.68),
            on_progress=events.append,
        )
        # A selected operation must leave the prepared timeline reusable.
        assert ready.render_frame_number(0).frame_number == 0
        again = ready.render_video(
            vestra.PreparedVideoRenderRequest(tmp_path / "again.mp4")
        )
        assert again.total_frames == 10
    else:
        result = value.render(
            clip, start=0.25, end=0.68, backend="cpu", on_progress=events.append
        )
    assert value.snapshot().to_dict() == before
    assert result.total_frames == 4
    assert result.duration_seconds == pytest.approx(0.4)
    assert result.performance.rendered_frame_count == 4
    assert events[0].total_frames == 4
    progress = [event for event in events if event.kind == "progress"]
    assert progress and progress[-1].frame == 4 and progress[-1].total_frames == 4
    decoded = pixels(clip)
    assert decoded.shape[0] == 4
    # Independent H.264 GOPs differ at edges; exact raw pixels are tested natively.
    difference = np.abs(decoded.astype(int) - pixels(full)[3:7].astype(int))
    assert difference.mean() <= 3 and difference.max() <= 24
    video = [frame for frame in probe(clip)["frames"] if frame["media_type"] == "video"]
    assert [float(frame["pts_time"]) for frame in video] == pytest.approx(
        [0, 0.1, 0.2, 0.3]
    )


@pytest.mark.parametrize(
    "start,end,count",
    [(0, 0.066733333, 2), (0.033366667, 0.066733334, 1), (0.04, None, 1)],
)
def test_encoded_range_uses_fractional_fps_grid(
    tmp_path: Path, start: float, end: float | None, count: int
):
    value = vestra.Project(size=(16, 16), fps=(30_000, 1001), duration=0.1)
    path = tmp_path / "fractional.mp4"
    result = value.render(
        path, start=start, end=end, backend="cpu", show_progress=False
    )
    video = [frame for frame in probe(path)["frames"] if frame["media_type"] == "video"]
    assert result.total_frames == count == len(video)
    assert result.duration_seconds == pytest.approx(count * 1001 / 30_000)
    assert float(video[0]["pts_time"]) == 0


@pytest.mark.parametrize(
    "factory", [vestra.RenderRequest, vestra.PreparedVideoRenderRequest]
)
def test_native_range_request_properties(factory):
    request = factory("output.mp4")
    assert request.start == 0 and request.end is None
    request = factory("output.mp4", start=0.5, end=1)
    assert request.start == 0.5 and request.end == 1
    for invalid in [-1, float("nan"), float("inf"), True, "1"]:
        with pytest.raises((TypeError, ValueError)):
            factory("output.mp4", start=invalid)
        with pytest.raises((TypeError, ValueError)):
            factory("output.mp4", end=invalid)


@pytest.mark.parametrize(
    "start,end", [(1, None), (0, 2), (0.5, 0.2), (0, 0), (0.01, 0.02)]
)
@pytest.mark.parametrize("prepared", [False, True])
def test_invalid_video_ranges_preserve_existing_destination(
    tmp_path: Path, start, end, prepared: bool
):
    path = tmp_path / "existing.mp4"
    path.write_bytes(b"original")
    value = project()
    with pytest.raises(vestra.RenderError) as caught:
        if prepared:
            value.prepare(backend="cpu").render_video(
                vestra.PreparedVideoRenderRequest(
                    path, start=start, end=end, overwrite=True
                )
            )
        else:
            value.render(
                path,
                start=start,
                end=end,
                backend="cpu",
                overwrite=True,
                show_progress=False,
            )
    assert caught.value.diagnostics[0].code == "VESTRA-RENDER-RANGE"
    assert path.read_bytes() == b"original"
    assert list(tmp_path.iterdir()) == [path]


def test_range_cancellation_reports_project_index_and_selected_progress(tmp_path: Path):
    path = tmp_path / "cancelled.mp4"
    path.write_bytes(b"original")
    token = vestra.CancellationToken()
    events: list[vestra.RenderEvent] = []

    def cancel(event: vestra.RenderEvent):
        events.append(event)
        if event.kind == "progress":
            token.cancel()

    with pytest.raises(vestra.CancelledError) as caught:
        project().render(
            path,
            start=0.3,
            end=0.7,
            backend="cpu",
            overwrite=True,
            cancellation=token,
            on_progress=cancel,
        )
    context = caught.value.failure_context
    assert context is not None
    assert context.total_frames == 4 and context.completed_frames == 1
    assert context.last_completed_frame_index == 3
    assert context.attempted_frame == 3
    assert context.timeline_position == pytest.approx(0.3)
    assert context.progress == pytest.approx(0.25)
    assert events[-1].kind == "cancelled"
    assert path.read_bytes() == b"original"
    assert list(tmp_path.iterdir()) == [path]


@pytest.mark.parametrize("dynamic", [False, True])
def test_video_range_trims_final_audio_after_original_time_automation(
    tmp_path: Path, dynamic: bool
):
    time = np.arange(48_000) / 48_000
    frequency = np.where(time < 0.5, 220, 880)
    samples = (16_000 * np.sin(2 * np.pi * frequency * time)).astype("<i2")
    source = tmp_path / "markers.wav"
    with wave.open(str(source), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(48_000)
        writer.writeframes(samples.tobytes())
    value = vestra.Project(size=(16, 16), fps=10, duration=1, output_audio=True)
    if dynamic:
        layer = value.root.add(vestra.sources.Color("#ff0000"))
        layer.opacity.keyframe(0, 0)
        layer.opacity.keyframe(1, 1)
    audio = value.audio.track("markers").add(source, trim_end=1)
    audio.set_gain_automation(
        [vestra.AudioGainKeyframe(0, 0), vestra.AudioGainKeyframe(1, 1)]
    )
    path = tmp_path / "audio-range.mp4"
    value.render(path, start=0.26, end=0.87, backend="cpu", show_progress=False)
    report = probe(path)
    audio_stream = next(
        stream for stream in report["streams"] if stream["codec_type"] == "audio"
    )
    assert float(audio_stream["start_time"]) == pytest.approx(0, abs=1 / 48_000)
    assert float(audio_stream["duration"]) == pytest.approx(0.6, abs=0.002)
    decoded = np.frombuffer(
        subprocess.check_output(
            [
                "ffmpeg",
                "-v",
                "error",
                "-i",
                str(path),
                "-map",
                "0:a:0",
                "-ac",
                "1",
                "-f",
                "f32le",
                "-",
            ]
        ),
        dtype="<f4",
    )
    for start, frequency, original_time in [(0.02, 220, 0.34), (0.42, 880, 0.74)]:
        window = decoded[round(start * 48_000) : round((start + 0.04) * 48_000)]
        frequencies = np.fft.rfftfreq(len(window), 1 / 48_000)
        measured = frequencies[np.argmax(np.abs(np.fft.rfft(window)))]
        assert measured == pytest.approx(frequency, abs=15)
        rms = np.sqrt(np.mean(window**2))
        assert rms == pytest.approx(
            16_000 / 32768 * original_time / np.sqrt(2), abs=0.015
        )


@pytest.mark.parametrize("dynamic", [False, True])
def test_rational_range_keeps_audio_through_final_partial_frame(
    tmp_path: Path, dynamic: bool
):
    path = tmp_path / "tail.mp4"
    source = tmp_path / "constant.wav"
    with wave.open(str(source), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(48_000)
        writer.writeframes(np.full(4800, 8192, dtype="<i2").tobytes())
    value = vestra.Project(size=(16, 16), fps=(30_000, 1001), duration=0.1)
    value.audio.track("tone").add(source, trim_end=0.1)
    if dynamic:
        layer = value.root.add(vestra.sources.Color("#ff0000"))
        layer.opacity.keyframe(0, 0)
        layer.opacity.keyframe(0.1, 1)
    value.render(path, start=0.05, end=0.1, backend="cpu", show_progress=False)
    streams = probe(path)["streams"]
    video = next(stream for stream in streams if stream["codec_type"] == "video")
    audio = next(stream for stream in streams if stream["codec_type"] == "audio")
    assert int(video["nb_frames"]) == 1
    # MP4 edit-list duration is rounded to milliseconds; AAC pads its final packet.
    assert float(audio["duration"]) == pytest.approx(1001 / 30_000, abs=0.001)
    decoded = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(path),
            "-map",
            "0:a:0",
            "-ac",
            "1",
            "-f",
            "f32le",
            "-",
        ]
    )
    assert len(decoded) // 4 >= 1602
