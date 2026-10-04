"""Render a deterministic audio-reactive reference scene."""

from __future__ import annotations

import math
import struct
import tempfile
import wave
from pathlib import Path

from vestra import (
    BackendPreference,
    Editor,
    FrameRate,
    PrepareOptions,
    PreparedVideoRenderRequest,
)
from vestra.authoring import ProjectBuilder, Sizing


SAMPLE_RATE = 48_000
DURATION = 4.0
ROOT = Path(__file__).resolve().parents[3]


def write_reference_audio(path: Path) -> None:
    frame_count = round(DURATION * SAMPLE_RATE)
    with wave.open(str(path), "wb") as stream:
        stream.setnchannels(1)
        stream.setsampwidth(2)
        stream.setframerate(SAMPLE_RATE)
        frames = bytearray()
        for index in range(frame_count):
            time = index / SAMPLE_RATE
            envelope = 0.35 if time >= DURATION / 2 else 0.08
            sample = envelope * math.sin(2 * math.pi * 100 * time) + 0.12 * math.sin(
                2 * math.pi * 4_000 * time
            )
            pcm = max(-32768, min(32767, round(sample * 32767)))
            frames.extend(struct.pack("<h", pcm))
        stream.writeframes(frames)


with tempfile.TemporaryDirectory(prefix="vestra-example-") as directory:
    output_dir = Path(directory)
    audio_path = output_dir / "audio-reactive-reference.wav"
    write_reference_audio(audio_path)

    builder = ProjectBuilder(
        width=320,
        height=180,
        frame_rate=FrameRate(30, 1),
        output_path="audio-reactive-reference.mp4",
        duration=DURATION,
        background="#101018",
        output_audio=False,
        base_directory=ROOT,
    )
    image = builder.add_image_asset("examples/assets/blue.png")
    clip = builder.add_image_clip(
        source=image,
        start=0,
        duration=DURATION,
        layer=0,
        sizing=Sizing.cover(),
    )
    asset = builder.add_audio_asset(str(audio_path), id="reference-master")

    builder.audio.add_track(id="music").add_clip(
        asset=asset, start=0, trim_end=DURATION
    )

    # Smooth the bass signal before using it to control visual properties.
    bass = (
        builder.audio.master.band(40, 160)
        .gain(20)
        .remap(0, 1, 0, 1)
        .clamp(0, 1)
        .envelope(attack=0.02, release=0.18)
        .response_curve(0.42, 0, 0.58, 1)
    )
    clip.transform.scale.react_to(bass.remap(0, 1, 1.0, 1.08), mode="multiply")
    clip.effects.add_glow(
        threshold=0.25,
        radius=8,
        intensity=0,
        colour="#FFFFFF",
    ).intensity.modulate(builder.audio.master.rms().gain(8), mode="replace")
    clip.effects.add_chromatic_aberration(
        amount=0,
        angle_degrees=0,
    ).amount.modulate(
        builder.audio.master.band(2_000, 12_000).gain(100), mode="replace"
    )

    project = builder.build()
    validation = builder.validate()
    assert validation.is_valid
    prepared = Editor().prepare(project, PrepareOptions(backend=BackendPreference.CPU))
    result = prepared.render_video(
        PreparedVideoRenderRequest(
            output_dir / "audio-reactive-reference.mp4", overwrite=True
        )
    )
    assert not result.audio_present
