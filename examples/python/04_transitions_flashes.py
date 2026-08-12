"""Create an explicitly fitted crossfade and a standalone flash."""
from vestra import FrameRate
from vestra.authoring import ProjectBuilder

builder = ProjectBuilder(width=320, height=180, frame_rate=FrameRate(30, 1), output_path="transitions.mp4")
asset = builder.add_image_asset("examples/assets/red.png")
outgoing = builder.add_image_clip(source=asset, start=0, duration=1, layer=0)
incoming = builder.add_image_clip(source=asset, start=0.75, duration=1, layer=1)
builder.timeline.add_crossfade_between(outgoing, incoming, duration=0.25)
builder.flashes.add(start=0.8, duration=0.1, colour="#ffffff", opacity=0.4, layer=2)
