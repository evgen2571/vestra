"""Use canonical presets and explicit timeline helpers.

Run from the repository root: uv run python examples/python/authoring/05_presets_timeline.py
"""

from vestra import FrameRate
from vestra.authoring import ProjectBuilder

builder = ProjectBuilder(
    width=320, height=180, frame_rate=FrameRate(30, 1), output_path="presets.mp4"
)

photo = builder.add_image_asset("assets/fixtures/red.png")
portrait = builder.add_image_clip(source=photo, start=0, duration=1, layer=0)
portrait.presets.apply_impact(seed=7, intensity=0.8)
builder.timeline.shift_clip(portrait, delta=0.25)
