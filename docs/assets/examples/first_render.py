"""Render a title card with Vestra. Run: python first_render.py"""

from pathlib import Path

from vestra import Ellipse, Project, Rectangle, Text

project = Project(
    size=(960, 540),
    fps=30,
    duration=3,
    background="#172c4b",
)

project.root.add(
    Ellipse(width=210, height=210, fill="#f07860"),
    position=(0.78, 0.47),
)

project.root.add(
    Rectangle(width=240, height=8, fill="#f5f7fb"),
    position=(0.78, 0.47),
    rotation_degrees=-25,
    z=1,
)

project.root.add(
    Text("Shape\nstudy", font_size=82, fill="#f5f7fb"),
    position=(0.12, 0.44),
    anchor=(0, 0.5),
    z=2,
)

project.root.add(
    Text("Position, scale, and opacity", font_size=22, fill="#b8c9e2"),
    position=(0.12, 0.79),
    anchor=(0, 0.5),
    z=2,
)

output = Path("first-render.mp4")
result = project.render(output, backend="cpu", overwrite=True)
print(f"Rendered {result.output_path}")
