from video_editor import FrameRate
from video_editor.authoring import Point, ProjectBuilder

builder = ProjectBuilder(width=4, height=4, frame_rate=FrameRate(1, 1), output_path="out.mp4")
clip = builder.add_solid_color_clip(colour="#000000", start=0, duration=1, layer=0)

sharpen = clip.effects.add_sharpen(amount=0.3, radius=1)
vignette = clip.effects.add_vignette(amount=0.3, radius=0.8, softness=0.4, colour="#000000")
chromatic = clip.effects.add_chromatic_aberration(amount=0.02, angle_degrees=0)

sharpen.angle_degrees
vignette.threshold
vignette.intensity
chromatic.radius
clip.effects.add_zoom_blur(radius=1, samples="8", anchor=Point(0.5, 0.5))
