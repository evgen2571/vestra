from video_editor import FrameRate
from video_editor.authoring import Point, ProjectBuilder

builder = ProjectBuilder(width=2, height=2, frame_rate=FrameRate(1, 1), output_path="out.mp4")
asset = builder.add_image_asset("image.png")
clip = builder.add_image_clip(source=asset, start=0, duration=1, layer=0)

clip.opacity = clip.opacity
clip.transform = clip.transform
clip.transform.position = clip.transform.position
clip.transform.scale = clip.transform.scale

# Mutating values, by contrast, remains supported.
clip.opacity.base_value = 0.5
clip.transform.scale.base_value = Point(1.2, 1.2)
