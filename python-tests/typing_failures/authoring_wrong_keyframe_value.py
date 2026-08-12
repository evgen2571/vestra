from vestra import FrameRate
from vestra.authoring import Point, ProjectBuilder

builder = ProjectBuilder(width=10, height=10, frame_rate=FrameRate(30, 1), output_path="out.mp4")
image = builder.add_image_asset("image.png")
clip = builder.add_image_clip(source=image, start=0, duration=1, layer=0)
clip.opacity.keyframe(time=0, value=Point(1, 1))
clip.transform.scale.keyframe(time=0, value=1.0)
clip.crop.keyframe(time=0, value=Point(0, 0))
clip.opacity.keyframes.append(1)
