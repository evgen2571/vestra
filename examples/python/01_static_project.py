"""Create a static project using only public authoring APIs."""
from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder, Sizing

builder = ProjectBuilder(width=320, height=180, frame_rate=FrameRate(30, 1), output_path="static.mp4")
asset = builder.add_image_asset("examples/assets/red.png")
builder.add_image_clip(source=asset, start=0, duration=1, layer=0, sizing=Sizing.cover())
assert builder.validate().is_valid
