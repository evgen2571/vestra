"""Compare interpolation modes. Run: python keyframes.py"""

from pathlib import Path

from vestra import Ellipse, Interpolation, Project, Rectangle, Text

project = Project(
    size=(960, 540),
    fps=30,
    duration=3,
    background="#172c4b",
)
project.root.add(
    Text("Position interpolation", font_size=35, fill="#f5f7fb"),
    position=(0.08, 0.13),
    anchor=(0, 0.5),
)

rows = [
    ("Linear", Interpolation.LINEAR, "#f5f7fb"),
    ("Ease out", Interpolation.EASE_OUT, "#f07860"),
    ("Ease in / out", Interpolation.EASE_IN_OUT, "#83b6ff"),
    ("Hold", Interpolation.HOLD, "#bedca2"),
]

for row_index, (label, interpolation, color) in enumerate(rows):
    row_y = 0.33 + row_index * 0.16

    project.root.add(
        Text(label, font_size=24, fill="#f5f7fb"),
        position=(0.08, row_y),
        anchor=(0, 0.5),
    )
    project.root.add(
        Rectangle(width=480, height=2, fill="#526985"),
        position=(0.63, row_y),
    )

    dot = project.root.add(
        Ellipse(width=26, height=26, fill=color),
        position=(0.38, row_y),
        z=1,
    )
    dot.transform.position.keyframe(0.3, (0.38, row_y))
    # The destination keyframe controls the interval from 0.3 to 2.3 seconds.
    dot.transform.position.keyframe(2.3, (0.88, row_y), interpolation=interpolation)

output = Path("keyframes.mp4")
result = project.render(output, backend="cpu", overwrite=True)
print(f"Rendered {result.output_path}")
