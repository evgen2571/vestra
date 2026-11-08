#!/usr/bin/env python3
"""Verify an installed wheel, including native APIs, bundled fonts and audio/video."""

from __future__ import annotations

import argparse
from importlib.metadata import distribution, version as distribution_version
import json
import math
import struct
import subprocess
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


def decode_aac(path: Path) -> list[float]:
    decoded = subprocess.run(
        [
            "ffmpeg",
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
    import os
    import sys

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
    probe = json.loads(
        subprocess.run(
            [
                "ffprobe",
                "-v",
                "error",
                "-count_frames",
                "-show_entries",
                "stream=codec_type,codec_name,sample_rate,channels,width,height,nb_read_frames:format=duration",
                "-of",
                "json",
                str(output),
            ],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    video = next(
        stream for stream in probe["streams"] if stream["codec_type"] == "video"
    )
    audio = next(
        stream for stream in probe["streams"] if stream["codec_type"] == "audio"
    )
    assert (video["codec_name"], video["width"], video["height"]) == ("h264", 32, 32)
    assert int(video["nb_read_frames"]) == 90
    assert abs(float(probe["format"]["duration"]) - 3.0) < 0.1
    assert (audio["codec_name"], audio["sample_rate"], audio["channels"]) == (
        "aac",
        "48000",
        2,
    )
    samples = decode_aac(output)
    early_440 = tone_correlation(samples, 440.0, 0.5, 0.4)
    early_880 = tone_correlation(samples, 880.0, 0.5, 0.4)
    late_440 = tone_correlation(samples, 440.0, 2.4, 0.4)
    late_880 = tone_correlation(samples, 880.0, 2.4, 0.4)
    assert early_440 > early_880, (early_440, early_880)
    assert late_880 > late_440, (late_880, late_440)
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
    args = parser.parse_args()

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
