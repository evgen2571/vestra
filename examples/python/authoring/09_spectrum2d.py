"""Render an audio-reactive Spectrum2D scene with radial bars and Bloom.

Run from the repository root: uv run python examples/python/authoring/09_spectrum2d.py
"""

from __future__ import annotations

import tempfile
from pathlib import Path

from vestra import BackendPreference, Editor, FrameRate, RenderRequest
from vestra.authoring import ProjectBuilder, Sizing


ROOT = Path(__file__).resolve().parents[3]

project = ProjectBuilder(
    width=320,
    height=180,
    frame_rate=FrameRate(30, 1),
    output_path="spectrum2d.mp4",
    duration=3.0,
    background="#101018",
    base_directory=ROOT,
)
background_asset = project.add_image_asset("assets/fixtures/green.png")
background = project.add_image_clip(
    source=background_asset,
    start=0,
    duration=3,
    layer=0,
    sizing=Sizing.cover(),
)
audio_asset = project.add_audio_asset("assets/fixtures/tone.wav")
audio_track = project.audio.add_track(id="music")

audio_track.add_clip(asset=audio_asset, start=0, trim_end=3)
# Analyse the master audio once and use it to animate the background.
loudness = project.audio.master.rms()
bounded_scale = loudness.remap(0, 1, 1, 1.08)
bass = bounded_scale.envelope(attack=0.02, release=0.15)
background.transform.scale.react_to(bass)
project.add_spectrum2d_clip(start=0, duration=3, layer=1, preset="neon_circle")

with tempfile.TemporaryDirectory(prefix="vestra-example-") as directory:
    Editor().render(
        project.build(),
        RenderRequest(
            Path(directory) / "spectrum2d.mp4",
            backend=BackendPreference.CPU,
            overwrite=True,
        ),
    )
