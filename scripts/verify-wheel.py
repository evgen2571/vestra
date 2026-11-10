#!/usr/bin/env python3
"""Verify an installed wheel, including native APIs, bundled fonts and audio/video."""

from __future__ import annotations

import argparse
from importlib.metadata import distribution, version as distribution_version
import math
import os
import re
import shutil
import struct
import subprocess
import sys
import tomllib
import tempfile
import wave
from contextlib import nullcontext
from pathlib import Path


SAMPLE_RATE = 48_000


def write_tone(path: Path, frequency: float) -> None:
    samples = bytearray()
    for index in range(SAMPLE_RATE * 3):
        value = int(
            0.35 * 32_767 * math.sin(2 * math.pi * frequency * index / SAMPLE_RATE)
        )
        samples.extend(struct.pack("<hh", value, value))
    with wave.open(str(path), "wb") as output:
        output.setnchannels(2)
        output.setsampwidth(2)
        output.setframerate(SAMPLE_RATE)
        output.writeframes(samples)


def decode_aac(path: Path, ffmpeg: str) -> list[float]:
    decoded = subprocess.run(
        [
            ffmpeg,
            "-v",
            "error",
            "-i",
            str(path),
            "-f",
            "f32le",
            "-ac",
            "2",
            "-ar",
            str(SAMPLE_RATE),
            "-",
        ],
        check=True,
        capture_output=True,
    ).stdout
    if not decoded:
        raise AssertionError("FFmpeg decoded no PCM from the rendered AAC stream")
    return list(struct.unpack(f"<{len(decoded) // 4}f", decoded))


def tone_correlation(
    samples: list[float], frequency: float, start: float, duration: float
) -> float:
    first = round(start * SAMPLE_RATE) * 2
    count = round(duration * SAMPLE_RATE)
    window = samples[first : first + count * 2]
    if len(window) != count * 2:
        raise AssertionError("decoded PCM does not cover the signal-analysis window")
    sine = cosine = energy = 0.0
    for index, sample in enumerate(window[::2]):
        phase = 2 * math.pi * frequency * index / SAMPLE_RATE
        sine += sample * math.sin(phase)
        cosine += sample * math.cos(phase)
        energy += sample * sample
    return math.hypot(sine, cosine) / math.sqrt(energy * count)


def decode_frames(
    path: Path,
    ffmpeg: str,
    width: int,
    height: int,
    count: int,
    fps: int | None = None,
) -> bytes:
    decoded = subprocess.run(
        [
            ffmpeg,
            "-hide_banner",
            "-i",
            str(path),
            "-map",
            "0:v:0",
            "-vf",
            "showinfo",
            "-vsync",
            "0",
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "-",
        ],
        capture_output=True,
        check=True,
    )
    assert len(decoded.stdout) == count * width * height * 4, path
    dimensions = re.findall(rb"\bs:(\d+)x(\d+)\b", decoded.stderr)
    assert dimensions == [(str(width).encode(), str(height).encode())] * count, path
    timestamps = [
        float(t) for t in re.findall(rb"pts_time:([-+\d.eE]+)", decoded.stderr)
    ]
    assert len(timestamps) == count and timestamps[0] == 0, path
    if fps is not None:
        assert all(
            abs(actual - index / fps) < 0.00001
            for index, actual in enumerate(timestamps)
        ), (path, timestamps)
    return decoded.stdout


def webp_frames(
    path: Path, width: int, height: int, count: int, fps: int
) -> list[bytes]:
    # Bundled FFmpeg can decode WebP stills but not every animated WebP container.
    data = path.read_bytes()
    assert data[:4] == b"RIFF" and data[8:12] == b"WEBP", path
    assert int.from_bytes(data[4:8], "little") + 8 == len(data), path
    frames = []
    offset = 12
    while offset < len(data):
        assert offset + 8 <= len(data), path
        tag, size = struct.unpack_from("<4sI", data, offset)
        body = data[offset + 8 : offset + 8 + size]
        assert len(body) == size, path
        if tag == b"ANMF":
            assert len(body) >= 16 and body[:6] == bytes(6), path
            assert int.from_bytes(body[6:9], "little") + 1 == width, path
            assert int.from_bytes(body[9:12], "little") + 1 == height, path
            assert int.from_bytes(body[12:15], "little") == 1000 // fps, path
            image = body[16:]
            frames.append(b"RIFF" + struct.pack("<I", len(image) + 4) + b"WEBP" + image)
        offset += 8 + size + size % 2
    assert offset == len(data) and len(frames) == count, path
    return frames


def verify_output_apis(output_dir: Path, ffmpeg: str) -> None:
    import vestra
    from vestra.authoring import Quality

    project = vestra.Project(size=(32, 16), fps=30, duration=2, background="#102030")
    layer = project.root.add(vestra.Color("#ff8040"))
    layer.opacity.keyframe(0, 0)
    layer.opacity.keyframe(2, 1)
    authored = project.snapshot().to_dict()
    prepared = project.prepare(
        backend="cpu", resolution=(16, 8), fps=vestra.FrameRate(4, 1)
    )
    frame = prepared.render_frame_number(4)
    expected = frame.to_bytes()
    assert (frame.width, frame.height, frame.frame_number) == (16, 8, 4)
    for extension in ("png", "jpg", "webp"):
        path = output_dir / f"still.{extension}"
        frame.save(path)
        pixels = decode_frames(path, ffmpeg, 16, 8, 1)
        tolerance = 4 if extension == "jpg" else 0
        assert max(abs(a - b) for a, b in zip(pixels, expected)) <= tolerance, path
    direct = project.render_frame(
        1, output_dir / "direct.png", backend="cpu", resolution=(16, 8), fps=4
    )
    assert direct.to_bytes() == expected
    sequence = output_dir / "sequence"
    assert (
        project.render_frames(
            directory=sequence,
            times=[1.5, 0.5, 1.5],
            backend="cpu",
            resolution=(16, 8),
            fps=(4, 1),
        )
        == 3
    )
    paths = sorted(sequence.glob("*.png"))
    assert [path.name for path in paths] == [
        f"frame_{index:06d}.png" for index in range(3)
    ]
    assert paths[0].read_bytes() == paths[2].read_bytes() != paths[1].read_bytes()
    for path in paths:
        decode_frames(path, ffmpeg, 16, 8, 1)

    try:
        import numpy as np
    except ModuleNotFoundError as error:
        if error.name != "numpy":
            raise
        print("Optional NumPy output check skipped: numpy is not installed")
    else:
        array = frame.to_numpy()
        assert array.shape == (8, 16, 4) and array.dtype == np.uint8
        assert array.strides == (64, 4, 1)
        assert (
            array.flags.owndata and array.flags.c_contiguous and array.flags.writeable
        )
        assert array.tobytes() == expected
        array[:] = 0
        prepared.render_frame_number(0)
        assert frame.to_bytes() == expected and not array.any()
        assert (
            project.get_frame(1, backend="cpu", resolution=(16, 8), fps=4).tobytes()
            == expected
        )

    selected = output_dir / "selected.mp4"
    result = project.render(
        selected,
        start=0.5,
        end=1.5,
        preview=True,
        resolution=(16, 8),
        fps=(4, 1),
        quality=Quality.HIGH,
        backend="cpu",
        show_progress=False,
    )
    assert (result.width, result.height, result.total_frames) == (16, 8, 4)
    assert result.duration_seconds == 1
    decoded = decode_frames(selected, ffmpeg, 16, 8, 4, 4)
    references = b"".join(
        prepared.render_frame_number(index).to_bytes() for index in range(2, 6)
    )
    assert max(abs(a - b) for a, b in zip(decoded, references)) <= 8
    preview = output_dir / "preview.mp4"
    result = project.render_preview(
        preview, start=0.5, end=1.5, backend="cpu", show_progress=False
    )
    assert (result.width, result.height, result.total_frames) == (32, 16, 15)
    decode_frames(preview, ffmpeg, 32, 16, 15, 15)

    for extension in ("gif", "webp"):
        animation = output_dir / f"animation.{extension}"
        result = project.render(
            animation,
            start=0.5,
            end=1.5,
            resolution=(16, 8),
            fps=4,
            backend="cpu",
            show_progress=False,
        )
        assert (result.width, result.height, result.total_frames) == (16, 8, 4)
        assert result.duration_seconds == 1
        if extension == "gif":
            pixels = decode_frames(animation, ffmpeg, 16, 8, 4, 4)
        else:
            pixels = bytearray()
            for index, image in enumerate(webp_frames(animation, 16, 8, 4, 4)):
                still = output_dir / f"webp-frame-{index}.webp"
                still.write_bytes(image)
                pixels.extend(decode_frames(still, ffmpeg, 16, 8, 1))
        assert max(abs(a - b) for a, b in zip(pixels, references)) <= 16, animation
    assert project.snapshot().to_dict() == authored
    print(
        "Wheel output APIs passed: stills, sequences, range/overrides, preview, GIF, animated WebP"
    )


def verify(output_dir: Path, expected_version: str | None = None) -> None:
    import vestra
    from vestra import (
        BackendPreference,
        Editor,
        FrameRate,
        InspectAudio,
        InspectAudioClip,
        InspectAudioGainKeyframe,
        InspectAudioTrack,
        PrepareOptions,
        PreparedVideoRenderRequest,
        ProjectSnapshot,
    )
    from vestra.authoring import (
        AudioFadeCurve,
        AudioGainInterpolation,
        AudioGainKeyframe,
        ProjectBuilder,
    )

    package_path = Path(vestra.__file__).resolve()
    if expected_version is None:
        workspace_manifest = Path(__file__).resolve().parents[1] / "Cargo.toml"
        workspace = tomllib.loads(workspace_manifest.read_text())
        expected_version = workspace["workspace"]["package"]["version"]
    assert distribution_version("vestra") == expected_version
    assert vestra.__version__ == expected_version
    assert vestra.native_version() == expected_version
    installed = distribution("vestra")
    assert installed.metadata["License-Expression"] == "MIT"
    requirement = installed.metadata["Requires-Python"].replace(" ", "")
    assert ">=3.11" in requirement and "<3.15" in requirement
    assert any(item.startswith("imageio-ffmpeg") for item in installed.requires or ())
    included = {Path(str(path)).name for path in installed.files or ()}
    assert {
        "LICENSE",
        "FFmpeg-LGPL-2.1.txt",
        "DejaVuSans.txt",
        "THIRD_PARTY_NOTICES.md",
        "_native.pyi",
    } <= included

    if "site-packages" not in package_path.parts:
        raise AssertionError(
            f"vestra did not import from the installed wheel: {package_path}"
        )
    if not (package_path.parent / "py.typed").is_file():
        raise AssertionError("the installed wheel is missing vestra/py.typed")

    # Run outside the source tree with empty discovery: packaged Text must stand alone.
    font_config = output_dir / "empty-fonts.conf"
    empty_fonts = output_dir / "empty-fonts"
    empty_fonts.mkdir(exist_ok=True)
    font_config.write_text(
        f'<?xml version="1.0"?><fontconfig><dir>{empty_fonts}</dir></fontconfig>'
    )
    subprocess.run(
        [
            sys.executable,
            "-c",
            """
from pathlib import Path
import vestra
p = vestra.Project(size=(160, 80), fps=1, duration=1)
p.root.add(vestra.Text("Hello Vestra Δ", font_size=24))
asset = Path(p.snapshot().to_dict()["assets"][0]["source"])
assert asset.is_file() and "site-packages" in asset.parts
assert asset.with_name("LICENSE.txt").is_file()
pixels = p.render_frame(0, backend="cpu").to_bytes()
assert any(pixels[i:i+3] != bytes(3) for i in range(0, len(pixels), 4))
""",
        ],
        cwd=output_dir,
        env=dict(os.environ, FONTCONFIG_FILE=str(font_config)),
        check=True,
    )

    tone_440 = output_dir / "tone-440.wav"
    tone_880 = output_dir / "tone-880.wav"
    output = output_dir / "wheel-smoke.mp4"
    write_tone(tone_440, 440.0)
    write_tone(tone_880, 880.0)

    builder = ProjectBuilder(
        width=32,
        height=32,
        frame_rate=FrameRate(30, 1),
        output_path=output.name,
        duration=3.0,
        output_audio=True,
        base_directory=output_dir,
    )
    builder.add_solid_color_clip(colour="#101018", start=0.0, duration=3.0, layer=0)
    # Preparation must find the bundled glyphs without repository or system fonts.
    builder.post_effects.add_ascii(characters=" .#", cell_width=4, cell_height=6)
    outgoing_asset = builder.add_audio_asset(tone_440.name)
    incoming_asset = builder.add_audio_asset(tone_880.name)
    music = builder.audio.add_track(id="music")
    outgoing = music.add_clip(asset=outgoing_asset, start=0.0, trim_end=2.0)
    incoming = music.add_clip(asset=incoming_asset, start=1.5, trim_end=1.5)
    incoming.set_gain_automation(
        [
            AudioGainKeyframe(0.0, 0.0, AudioGainInterpolation.LINEAR),
            AudioGainKeyframe(0.25, 1.0, AudioGainInterpolation.HOLD),
            AudioGainKeyframe(0.5, 0.8, AudioGainInterpolation.LINEAR),
        ]
    )
    builder.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)

    validation = builder.validate()
    assert validation.is_valid and not validation.warnings
    project = builder.build()
    data = project.to_dict()
    assert ProjectSnapshot.from_dict(data, base_directory=output_dir).to_dict() == data
    assert (
        ProjectSnapshot.from_json(
            project.to_json(), base_directory=output_dir
        ).to_dict()
        == data
    )
    inspection = Editor().inspect(project)
    assert isinstance(inspection.audio, InspectAudio)
    assert isinstance(inspection.audio.tracks[0], InspectAudioTrack)
    inspected_incoming = inspection.audio.tracks[0].clips[1]
    assert isinstance(inspected_incoming, InspectAudioClip)
    assert isinstance(inspected_incoming.gain_automation[0], InspectAudioGainKeyframe)
    assert len(inspected_incoming.gain_automation) == 3

    prepared = Editor().prepare(project, PrepareOptions(backend=BackendPreference.CPU))
    assert prepared.preparation_report.selected_backend == vestra.BackendKind.CPU
    frame = prepared.render_frame_number(0)
    assert (frame.width, frame.height, frame.frame_number) == (32, 32, 0)
    assert frame.pixel_format == vestra.PixelFormat.RGBA8
    assert len(frame.to_bytes()) == 32 * 32 * 4
    assert frame.to_bytes() == bytes(frame)
    events = []
    result = prepared.render_video(
        PreparedVideoRenderRequest(output), on_progress=events.append
    )
    assert result.audio_present
    assert result.selected_backend == "cpu"
    assert events and all(isinstance(event, vestra.RenderEvent) for event in events)
    from vestra._ffmpeg import resolve_ffmpeg

    ffmpeg = resolve_ffmpeg()
    print(
        subprocess.run(
            [ffmpeg, "-version"], capture_output=True, text=True, check=True
        ).stdout
    )
    print(
        subprocess.run(
            [ffmpeg, "-L"], capture_output=True, text=True, check=True
        ).stdout
    )
    print(f"Installed FFmpeg executable: {Path(ffmpeg).stat().st_size} bytes")
    metadata = subprocess.run(
        [ffmpeg, "-hide_banner", "-i", str(output)],
        capture_output=True,
        text=True,
    ).stderr
    assert "Video: h264" in metadata and "32x32" in metadata, metadata
    assert "Audio: aac" in metadata and "48000 Hz, stereo" in metadata, metadata
    assert "Duration: 00:00:03.00" in metadata, metadata
    decoded = subprocess.run(
        [
            ffmpeg,
            "-hide_banner",
            "-i",
            str(output),
            "-map",
            "0:v:0",
            "-vf",
            "showinfo",
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "-",
        ],
        capture_output=True,
        check=True,
    )
    assert len(decoded.stdout) == 90 * 32 * 32 * 4
    timestamps = [float(t) for t in re.findall(rb"pts_time:([\d.]+)", decoded.stderr)]
    assert len(timestamps) == 90 and timestamps[0] == 0
    assert all(
        abs(actual - frame / 30) < 0.00001 for frame, actual in enumerate(timestamps)
    )
    samples = decode_aac(output, ffmpeg)
    assert abs(len(samples) // 2 - 3 * SAMPLE_RATE) <= 1024
    early_440 = tone_correlation(samples, 440.0, 0.5, 0.4)
    early_880 = tone_correlation(samples, 880.0, 0.5, 0.4)
    late_440 = tone_correlation(samples, 440.0, 2.4, 0.4)
    late_880 = tone_correlation(samples, 880.0, 2.4, 0.4)
    middle_440 = tone_correlation(samples, 440.0, 1.65, 0.1)
    middle_880 = tone_correlation(samples, 880.0, 1.65, 0.1)
    assert early_440 > 0.5 and early_440 > early_880, (early_440, early_880)
    assert middle_440 > 0.05 and middle_880 > 0.05, (middle_440, middle_880)
    assert late_880 > 0.5 and late_880 > late_440, (late_880, late_440)
    verify_output_apis(output_dir, ffmpeg)

    # Reload the published MP4 through the native libraries, then exercise raw
    # encoding and master analysis with the same no-system-tools environment.
    project = vestra.Project(
        size=(32, 32), fps=30, duration=3, base_directory=output_dir
    )
    layer = project.root.add(vestra.Video(output))
    project.audio.track().add(tone_440)
    layer.opacity.bind(project.audio.signal.rms(), operation="multiply")
    session = project.prepare(backend="cpu")
    assert any(session.render_frame_seconds(1).to_bytes())
    rendered = session.render_video(
        PreparedVideoRenderRequest(output_dir / "decoded.mp4")
    )
    assert rendered.audio_present and rendered.total_frames == 90

    # A user binary with spaces in its path must also reach one-shot/static
    # encoding. Prepared sessions retain their original executable selection.
    custom = output_dir / ("custom ffmpeg.exe" if os.name == "nt" else "custom ffmpeg")
    shutil.copy2(ffmpeg, custom)
    os.environ["IMAGEIO_FFMPEG_EXE"] = str(custom)
    static = vestra.Project(size=(16, 16), fps=2, duration=1)
    static.root.add(vestra.Color("#204060"))
    static.audio.track().add(tone_880, trim_end=1)
    assert static.render(
        output_dir / "override.mp4", backend="cpu", show_progress=False
    ).audio_present
    os.environ["IMAGEIO_FFMPEG_EXE"] = str(output_dir / "missing-ffmpeg")
    session.render_video(PreparedVideoRenderRequest(output_dir / "retained.mp4"))
    try:
        static.render(output_dir / "invalid.mp4", backend="cpu", show_progress=False)
    except vestra.RenderError as error:
        assert any(
            item.code == "VESTRA-ENCODER-UNAVAILABLE" for item in error.diagnostics
        )
    else:
        raise AssertionError("invalid explicit FFmpeg selection was ignored")
    assert not (output_dir / "invalid.mp4").exists()
    print(
        f"FFmpeg executable: {ffmpeg}; dependency: {distribution_version('imageio-ffmpeg')}"
    )
    print(
        "wheel smoke passed: "
        f"early 440={early_440:.3f} > 880={early_880:.3f}; "
        f"late 880={late_880:.3f} > 440={late_440:.3f}"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output-dir", type=Path, help="keep the rendered smoke artifact here"
    )
    parser.add_argument(
        "--expected-version",
        help="expected release version; defaults to the checkout version",
    )
    parser.add_argument("--isolated", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()

    if not args.isolated:
        # Import the extension only in a child with no system media tools or
        # development library search paths. An installed dependency is enough.
        with tempfile.TemporaryDirectory(prefix="vestra-empty-path-") as empty_path:
            environment = dict(os.environ, PATH=empty_path)
            for key in (
                "IMAGEIO_FFMPEG_EXE",
                "LD_LIBRARY_PATH",
                "DYLD_LIBRARY_PATH",
                "PYTHONPATH",
            ):
                environment.pop(key, None)
            subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    *sys.argv[1:],
                    "--isolated",
                ],
                env=environment,
                check=True,
            )
        return

    assert shutil.which("ffmpeg") is None and shutil.which("ffprobe") is None

    context = (
        nullcontext(args.output_dir)
        if args.output_dir is not None
        else tempfile.TemporaryDirectory(prefix="vestra-wheel-")
    )
    with context as directory:
        output_dir = Path(directory).resolve()
        output_dir.mkdir(parents=True, exist_ok=True)
        verify(output_dir, args.expected_version)


if __name__ == "__main__":
    main()
