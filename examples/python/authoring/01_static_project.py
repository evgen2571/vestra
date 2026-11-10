"""Create a static project using only public authoring APIs.

Run from the repository root: uv run python examples/python/authoring/01_static_project.py
"""

from vestra import FrameRate
from vestra.authoring import ProjectBuilder, Sizing

builder = ProjectBuilder(
    width=320, height=180, frame_rate=FrameRate(30, 1), output_path="static.mp4"
)

photo = builder.add_image_asset("assets/fixtures/red.png")
builder.add_image_clip(
    source=photo, start=0, duration=1, layer=0, sizing=Sizing.cover()
)
assert builder.validate().is_valid
