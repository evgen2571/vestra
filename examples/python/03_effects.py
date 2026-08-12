"""Apply ordered clip and global effects."""
from vestra import FrameRate
from vestra.authoring import ProjectBuilder

builder = ProjectBuilder(width=320, height=180, frame_rate=FrameRate(30, 1), output_path="effects.mp4")
asset = builder.add_image_asset("examples/assets/red.png")
clip = builder.add_image_clip(source=asset, start=0, duration=1, layer=0)
clip.effects.add_brightness(amount=0.1)
builder.post_effects.add_vignette(amount=0.2, radius=0.8, softness=0.3, colour="#000000")
