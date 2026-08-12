from vestra import FrameRate
from vestra.authoring import ProjectBuilder

builder = ProjectBuilder(width=4, height=4, frame_rate=FrameRate(1, 1), output_path="out.mp4")
builder.post_effects.add_camera_shake(
    position_amount=0.01,
    rotation_degrees=1.0,
    scale_amount=0.01,
    frequency=12.0,
    seed=1,
    attack=0.0,
    decay=0.1,
)
