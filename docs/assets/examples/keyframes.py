"""Compare position interpolation modes. Run: python keyframes.py"""

from pathlib import Path

from vestra import Interpolation, Project, Rectangle, Text

project = Project(
    size=(960, 540),
    fps=30,
    duration=3,
    background="#101c24",
)
project.root.add(
    Text("Position interpolation", font_size=35, fill="#eff6f2"),
    position=(0.08, 0.13),
    anchor=(0, 0.5),
)

rows = [
    ("Linear", Interpolation.LINEAR),
    ("Ease out", Interpolation.EASE_OUT),
    ("Ease in / out", Interpolation.EASE_IN_OUT),
    ("Hold", Interpolation.HOLD),
]

for row_index, (label, interpolation) in enumerate(rows):
    row_y = 0.33 + row_index * 0.16

    project.root.add(
        Text(label, font_size=24, fill="#eff6f2"),
        position=(0.08, row_y),
        anchor=(0, 0.5),
    )
    project.root.add(
        Rectangle(width=480, height=2, fill="#44675a"),
        position=(0.63, row_y),
    )

    card = project.root.add(
        Rectangle(width=32, height=26, corner_radius=5, fill="#b8e3bc"),
        position=(0.38, row_y),
        z=1,
    )
    card.transform.position.keyframe(0.3, (0.38, row_y))
    # The destination keyframe controls the interval from 0.3 to 2.3 seconds.
    card.transform.position.keyframe(2.3, (0.88, row_y), interpolation=interpolation)

output = Path("keyframes.mp4")
result = project.render(output, backend="cpu", overwrite=True)
print(f"Rendered {result.output_path}")
