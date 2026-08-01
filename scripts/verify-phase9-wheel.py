#!/usr/bin/env python3
"""Exercise the installed wheel through the final public Phase 9 workflow."""

from __future__ import annotations

import argparse
import math
import struct
import subprocess
import tempfile
import wave
from pathlib import Path


SAMPLE_RATE = 48_000


def write_tone(path: Path, frequency: float) -> None:
    samples = bytearray()
    for index in range(SAMPLE_RATE * 3):
        value = int(0.35 * 32_767 * math.sin(2 * math.pi * frequency * index / SAMPLE_RATE))
        samples.extend(struct.pack("<hh", value, value))
    with wave.open(str(path), "wb") as output:
        output.setnchannels(2)
        output.setsampwidth(2)
        output.setframerate(SAMPLE_RATE)
        output.writeframes(samples)


def decode_aac(path: Path) -> list[float]:
    decoded = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ac", "2", "-ar", str(SAMPLE_RATE), "-"],
        check=True,
        capture_output=True,
    ).stdout
    if not decoded:
        raise AssertionError("FFmpeg decoded no PCM from the rendered AAC stream")
    return list(struct.unpack(f"<{len(decoded) // 4}f", decoded))


def tone_correlation(samples: list[float], frequency: float, start: float, duration: float) -> float:
    first = round(start * SAMPLE_RATE) * 2
    count = round(duration * SAMPLE_RATE)
    window = samples[first:first + count * 2]
    if len(window) != count * 2:
        raise AssertionError("decoded PCM does not cover the signal-analysis window")
    sine = cosine = energy = 0.0
    for index, sample in enumerate(window[::2]):
        phase = 2 * math.pi * frequency * index / SAMPLE_RATE
        sine += sample * math.sin(phase)
        cosine += sample * math.cos(phase)
        energy += sample * sample
    return math.hypot(sine, cosine) / math.sqrt(energy * count)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", type=Path, help="directory for the rendered smoke artifact")
    args = parser.parse_args()

    import video_editor
    from video_editor import (
        BackendPreference,
        Editor,
        FrameRate,
        InspectAudio,
        InspectAudioClip,
        InspectAudioGainKeyframe,
        InspectAudioTrack,
        PrepareOptions,
        PreparedVideoRenderRequest,
    )
    from video_editor.authoring import AudioFadeCurve, AudioGainInterpolation, AudioGainKeyframe, ProjectBuilder

    package_path = Path(video_editor.__file__).resolve()
    if "site-packages" not in package_path.parts:
        raise AssertionError(f"video_editor did not import from the installed wheel: {package_path}")
    if not (package_path.parent / "py.typed").is_file():
        raise AssertionError("the installed wheel is missing video_editor/py.typed")

    output_dir = args.output_dir or Path(tempfile.mkdtemp(prefix="video-editor-phase9-wheel-"))
    output_dir.mkdir(parents=True, exist_ok=True)
    tone_440 = output_dir / "tone-440.wav"
    tone_880 = output_dir / "tone-880.wav"
    output = output_dir / "phase9-wheel.mp4"
    write_tone(tone_440, 440.0)
    write_tone(tone_880, 880.0)

    builder = ProjectBuilder(
        width=32, height=32, frame_rate=FrameRate(30, 1), output_path="phase9-wheel.mp4",
        duration=3.0, output_audio=True, base_directory=output_dir,
    )
    builder.add_solid_color_clip(colour="#101018", start=0.0, duration=3.0, layer=0)
    outgoing_asset = builder.add_audio_asset(tone_440.name)
    incoming_asset = builder.add_audio_asset(tone_880.name)
    music = builder.audio.add_track(id="music")
    outgoing = music.add_clip(asset=outgoing_asset, start=0.0, trim_end=2.0)
    incoming = music.add_clip(asset=incoming_asset, start=1.5, trim_end=1.5)
    incoming.set_gain_automation([
        AudioGainKeyframe(0.0, 0.0, AudioGainInterpolation.LINEAR),
        AudioGainKeyframe(0.25, 1.0, AudioGainInterpolation.HOLD),
        AudioGainKeyframe(0.5, 0.8, AudioGainInterpolation.LINEAR),
    ])
    builder.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)

    validation = builder.validate()
    assert validation.is_valid and not validation.warnings
    project = builder.build()
    inspection = Editor().inspect(project)
    assert isinstance(inspection.audio, InspectAudio)
    assert isinstance(inspection.audio.tracks[0], InspectAudioTrack)
    inspected_incoming = inspection.audio.tracks[0].clips[1]
    assert isinstance(inspected_incoming, InspectAudioClip)
    assert isinstance(inspected_incoming.gain_automation[0], InspectAudioGainKeyframe)
    assert len(inspected_incoming.gain_automation) == 3

    prepared = Editor().prepare(project, PrepareOptions(backend=BackendPreference.CPU))
    result = prepared.render_video(PreparedVideoRenderRequest(output))
    assert result.audio_present
    streams = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries", "stream=codec_name,sample_rate,channels", "-of", "default=noprint_wrappers=1", str(output)],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    assert "codec_name=aac" in streams and "sample_rate=48000" in streams and "channels=2" in streams
    samples = decode_aac(output)
    early_440 = tone_correlation(samples, 440.0, 0.5, 0.4)
    early_880 = tone_correlation(samples, 880.0, 0.5, 0.4)
    late_440 = tone_correlation(samples, 440.0, 2.4, 0.4)
    late_880 = tone_correlation(samples, 880.0, 2.4, 0.4)
    assert early_440 > early_880, (early_440, early_880)
    assert late_880 > late_440, (late_880, late_440)
    print(f"wheel smoke passed: early 440={early_440:.3f} > 880={early_880:.3f}; late 880={late_880:.3f} > 440={late_440:.3f}")


if __name__ == "__main__":
    main()
