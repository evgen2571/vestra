"""Animate a clip-local transform track.

Run from the repository root: uv run python examples/python/authoring/02_animation.py
"""

from vestra import FrameRate
from vestra.authoring import Interpolation, Point, ProjectBuilder

builder = ProjectBuilder(
    width=320, height=180, frame_rate=FrameRate(30, 1), output_path="animation.mp4"
)

photo = builder.add_image_asset("assets/fixtures/red.png")
portrait = builder.add_image_clip(source=photo, start=0, duration=1, layer=0)
# Keyframe times are local to this clip, measured in seconds.
portrait.transform.scale.keyframe(time=0.0, value=Point(1, 1))
portrait.transform.scale.keyframe(
    time=1.0, value=Point(1.2, 1.2), interpolation=Interpolation.EASE_OUT
)
