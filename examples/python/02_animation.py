"""Animate a clip-local transform track."""
from vestra import FrameRate
from vestra.authoring import Interpolation, Point, ProjectBuilder

builder = ProjectBuilder(width=320, height=180, frame_rate=FrameRate(30, 1), output_path="animation.mp4")
asset = builder.add_image_asset("examples/assets/red.png")
clip = builder.add_image_clip(source=asset, start=0, duration=1, layer=0)
clip.transform.scale.keyframe(time=0.0, value=Point(1, 1))
clip.transform.scale.keyframe(time=1.0, value=Point(1.2, 1.2), interpolation=Interpolation.EASE_OUT)
