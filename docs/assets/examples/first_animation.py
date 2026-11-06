"""Animate the title card. Run: python first_animation.py"""

from pathlib import Path

from vestra import Ellipse, Interpolation, Project, Rectangle, Text

project = Project(
    size=(960, 540),
    fps=30,
    duration=3,
    background="#172c4b",
)

circle = project.root.add(
    Ellipse(width=210, height=210, fill="#f07860"),
    position=(0.78, 0.47),
)
circle.transform.scale.keyframe(0, 0.4)
circle.transform.scale.keyframe(0.8, 1, interpolation=Interpolation.EASE_OUT)

stripe = project.root.add(
    Rectangle(width=240, height=8, fill="#f5f7fb"),
    position=(0.78, 0.47),
    rotation_degrees=-25,
    z=1,
)
stripe.transform.rotation_degrees.keyframe(0, -70)
stripe.transform.rotation_degrees.keyframe(
    0.8, -25, interpolation=Interpolation.EASE_OUT
)

headline = project.root.add(
    Text("Shape\nstudy", font_size=82, fill="#f5f7fb"),
    position=(0.12, 0.44),
    anchor=(0, 0.5),
    opacity=0,
    z=2,
)
headline.transform.position.keyframe(0, (0.12, 0.53))
headline.transform.position.keyframe(
    0.8, (0.12, 0.44), interpolation=Interpolation.EASE_OUT
)
headline.opacity.keyframe(0, 0)
headline.opacity.keyframe(0.6, 1)

caption = project.root.add(
    Text("Position, scale, and opacity", font_size=22, fill="#b8c9e2"),
    position=(0.12, 0.79),
    anchor=(0, 0.5),
    opacity=0,
    z=2,
)
# An explicit first keyframe keeps the caption hidden until 0.5 seconds.
caption.opacity.keyframe(0.5, 0)
caption.opacity.keyframe(1.1, 1)

output = Path("first-animation.mp4")
result = project.render(output, backend="cpu", overwrite=True)
print(f"Rendered {result.output_path}")
