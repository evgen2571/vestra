from vestra import FrameRate
from vestra.authoring import ProjectBuilder

builder = ProjectBuilder(width=4, height=4, frame_rate=FrameRate(30, 1), output_path="out.mp4", duration=1)
asset = builder.add_image_asset("cover.png")
image = builder.add_image_clip(source=asset, start=0, duration=1, layer=0)
solid = builder.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0)
builder.transitions.add_crossfade(outgoing=solid, incoming=image, start=0, duration=1)
builder.timeline.add_crossfade_between(solid, image, duration=1)
solid.presets.apply_zoom_punch()
image.id = "other"
builder.transitions = object()
image.presets = object()
builder.timeline = object()
