from video_editor import FrameRate
from video_editor.authoring import ProjectBuilder

builder = ProjectBuilder(
    width=16,
    height=16,
    frame_rate=FrameRate(24, 1),
    output_path="out.mp4",
)
image = builder.add_image_asset("image.png")
clip = builder.add_image_clip(source=image, start=0, duration=1, layer=0)
signal = builder.audio.master.rms()

# Transform component targets are modifier-only: authored animation belongs to
# the parent PointTrack (`position` / `scale`).
clip.transform.position_x.keyframe(time=0, value=1)
clip.transform.position_x.base_value = 1.0

# Deferred scalar properties remain ordinary ScalarTrack values and therefore
# do not advertise signal modulation to static typing.
vignette = clip.effects.add_vignette(
    amount=0,
    radius=1,
    softness=0.5,
    colour="#000000",
)
vignette.softness.modulate(signal)

adjust = clip.effects.add_color_adjust(
    exposure=0,
    gamma=1,
    black_point=0,
    white_point=1,
)
adjust.black_point.modulate(signal)
adjust.white_point.react_to(signal)
