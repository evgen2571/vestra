import json
import os
from pathlib import Path
import struct
import subprocess
import wave

import numpy as np
import pytest

import vestra


def project() -> vestra.Project:
    value = vestra.Project(size=(16, 16), fps=10, duration=1, background="#102030")
    layer = value.root.add(vestra.Rectangle(width=4, height=4, fill="#ef3050"))
    layer.transform.position.keyframe(0, (0.25, 0.5))
    layer.transform.position.keyframe(1, (0.75, 0.5))
    return value


def decode(path: Path) -> bytes:
    return subprocess.check_output(
        ["ffmpeg", "-v", "error", "-i", str(path), "-fps_mode", "passthrough",
         "-f", "rawvideo", "-pix_fmt", "rgba", "-"]
    )


def webp_frames(path: Path, tmp_path: Path) -> tuple[list[bytes], list[int]]:
    # FFmpeg builds before 8 lack animated-WebP decoding; each VP8L frame is portable.
    data = path.read_bytes()
    assert data[:4] == b"RIFF" and data[8:12] == b"WEBP"
    assert struct.unpack_from("<I", data, 4)[0] + 8 == len(data)
    frames, delays = [], []
    offset = 12
    while offset < len(data):
        size = struct.unpack_from("<I", data, offset + 4)[0]
        payload = data[offset + 8:offset + 8 + size]
        if data[offset:offset + 4] == b"ANMF":
            assert payload[15] == 2  # full-canvas replacement, including transparent pixels
            delays.append(int.from_bytes(payload[12:15], "little"))
            chunks = payload[16:]
            still = tmp_path / "decoded-frame.webp"
            still.write_bytes(b"RIFF" + struct.pack("<I", len(chunks) + 4) + b"WEBP" + chunks)
            frames.append(decode(still))
            still.unlink()
        offset += 8 + size + size % 2
    return frames, delays


@pytest.mark.parametrize("extension", ["gif", "webp"])
@pytest.mark.parametrize("prepared", [False, True])
def test_animation_range_preserves_selected_timeline_and_timing(
    tmp_path: Path, extension: str, prepared: bool
):
    value = project()
    before = value.snapshot().to_dict()
    ready = value.prepare(backend="cpu")
    expected = [ready.render_frame_number(index).to_bytes() for index in range(3, 7)]
    path = tmp_path / f"clip.{extension}"
    events: list[vestra.RenderEvent] = []
    if prepared:
        result = ready.render_video(
            vestra.PreparedVideoRenderRequest(path, start=0.25, end=0.68),
            on_progress=events.append,
        )
        assert ready.render_frame_number(0).frame_number == 0
    else:
        result = value.render(path, start=0.25, end=0.68, backend="cpu", on_progress=events.append)
    assert value.snapshot().to_dict() == before
    assert result.total_frames == 4
    assert result.duration_seconds == pytest.approx(0.4)
    assert not result.audio_present
    assert events[-1].kind == "completed"
    if extension == "webp":
        frames, delays = webp_frames(path, tmp_path)
        assert delays == [100] * 4
    else:
        raw = decode(path)
        frames = [raw[index:index + 16 * 16 * 4] for index in range(0, len(raw), 16 * 16 * 4)]
        probe = json.loads(subprocess.check_output(
            ["ffprobe", "-v", "error", "-show_frames", "-show_streams", "-of", "json", str(path)]
        ))
        assert len(probe["streams"]) == 1
        assert [float(frame["pts_time"]) for frame in probe["frames"]] == pytest.approx([0, 0.1, 0.2, 0.3])
    assert len(frames) == len(expected)
    for actual, source in zip(frames, expected, strict=True):
        assert np.array_equal(np.frombuffer(actual, dtype=np.uint8), np.frombuffer(source, dtype=np.uint8))


@pytest.mark.parametrize("extension", ["gif", "webp"])
def test_static_animation_reuses_one_frame_and_cleans_cancelled_overwrite(tmp_path: Path, extension: str):
    value = vestra.Project(size=(16, 16), fps=10, duration=0.3, background="#204080")
    path = tmp_path / f"static.{extension}"
    result = value.render(path, backend="cpu", show_progress=False)
    assert result.total_frames == 3
    assert result.performance.rendered_frame_count == 3
    existing = path.read_bytes()
    token = vestra.CancellationToken()

    def cancel(event: vestra.RenderEvent):
        if event.kind == "progress":
            token.cancel()

    with pytest.raises(vestra.CancelledError) as captured:
        value.render(path, backend="cpu", overwrite=True, cancellation=token, on_progress=cancel)
    assert captured.value.temporary_removed
    assert path.read_bytes() == existing
    assert list(tmp_path.iterdir()) == [path]


def snapshot_with_missing_audio(tmp_path: Path) -> vestra.ProjectSnapshot:
    return vestra.ProjectSnapshot.from_dict({
        "schema_version": 1,
        "output": {"path": "unused.mp4", "width": 16, "height": 16, "frame_rate": "10/1",
                   "background": "#102030", "quality": "high", "audio": True,
                   "duration_mode": "explicit", "duration": 0.3},
        "assets": [{"id": "missing", "type": "audio", "source": "missing.wav"}],
        "audio": {"tracks": [{"id": "music", "clips": [{"id": "tone", "asset": "missing",
                                                        "start": 0, "trim_start": 0, "trim_end": 0.3}]}]},
        "visual": {"clips": []},
    }, base_directory=tmp_path)


@pytest.mark.parametrize("extension", ["gif", "webp"])
def test_animation_omits_audio_and_retains_visual_audio_analysis(tmp_path: Path, extension: str):
    source = tmp_path / "constant.wav"
    with wave.open(str(source), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(48_000)
        writer.writeframes(np.full(48_000, 8192, dtype="<i2").tobytes())
    value = vestra.Project(size=(16, 16), fps=10, duration=1, output_audio=True)
    value.audio.track("tone").add(source, trim_end=1)
    layer = value.root.add(vestra.sources.Color("#ffffff"))
    layer.opacity.bind(value.audio.signal.rms(), operation="replace")
    ready = value.prepare(backend="cpu")
    expected = [ready.render_frame_number(index).to_bytes() for index in [1, 2]]
    assert 0 < expected[0][0] < 255
    before = value.snapshot().to_dict()
    path = tmp_path / f"reactive.{extension}"
    result = value.render(path, start=0.1, end=0.3, backend="cpu", show_progress=False)
    assert not result.audio_present and result.total_frames == 2
    assert value.snapshot().to_dict() == before
    if extension == "webp":
        frames, _ = webp_frames(path, tmp_path)
    else:
        raw = decode(path)
        frames = [raw[index:index + 16 * 16 * 4] for index in range(0, len(raw), 16 * 16 * 4)]
        report = json.loads(subprocess.check_output(
            ["ffprobe", "-v", "error", "-show_streams", "-of", "json", str(path)]
        ))
        assert len(report["streams"]) == 1
    assert len(frames) == 2
    assert frames == expected


@pytest.mark.parametrize("extension", ["gif", "webp", "mp4"])
def test_render_preflight_keeps_declared_asset_validation(tmp_path: Path, extension: str):
    snapshot = snapshot_with_missing_audio(tmp_path)
    before = snapshot.to_dict()
    report = vestra.Editor().preflight(snapshot, vestra.PreflightOptions.for_render(
        vestra.BackendPreference.CPU, tmp_path / f"output.{extension}",
    ))
    assert snapshot.to_dict() == before
    assert not report.is_ready
    assert any("missing.wav" in diagnostic.message for diagnostic in report.errors)
    with wave.open(str(tmp_path / "missing.wav"), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(48_000)
        writer.writeframes(np.zeros(14_400, dtype="<i2").tobytes())
    ready = vestra.Editor().preflight(snapshot, vestra.PreflightOptions.for_render(
        vestra.BackendPreference.CPU, tmp_path / f"output.{extension}",
    ))
    assert ready.is_ready, ready.errors


@pytest.mark.skipif(os.name == "nt", reason="uses a POSIX fake FFmpeg executable")
def test_missing_animation_encoder_is_explicit_and_preserves_existing_output(tmp_path: Path, monkeypatch):
    executable = tmp_path / "ffmpeg"
    executable.write_text("#!/bin/sh\ncase \"$*\" in\n *-version*) exit 0;;\n *-encoders*) printf ' V..... libwebp_anim fake animation encoder\\n'; exit 0;;\nesac\nexit 1\n")
    executable.chmod(0o755)
    monkeypatch.setenv("IMAGEIO_FFMPEG_EXE", str(executable))
    path = tmp_path / "existing.webp"
    path.write_bytes(b"existing")
    with pytest.raises(vestra.RenderError, match="libwebp.*unavailable"):
        project().render(path, backend="cpu", overwrite=True, show_progress=False)
    assert path.read_bytes() == b"existing"
    assert sorted(tmp_path.iterdir()) == sorted([path, executable])
