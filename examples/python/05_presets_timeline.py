"""Use canonical presets and explicit timeline helpers."""
from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder

builder = ProjectBuilder(width=320, height=180, frame_rate=FrameRate(30, 1), output_path="presets.mp4")
asset = builder.add_image_asset("examples/assets/red.png")
clip = builder.add_image_clip(source=asset, start=0, duration=1, layer=0)
clip.presets.apply_impact(seed=7, intensity=0.8)
builder.timeline.shift_clip(clip, delta=0.25)
