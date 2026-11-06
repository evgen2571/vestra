"""A static version of the README quickstart; no external assets required."""

from vestra import Project
from vestra.sources import Color, Rectangle

project = Project(size=(640, 360), fps=30, duration=3)
project.root.add(Color("#101c24"))

card = project.root.add(
    Rectangle(width=180, height=140, corner_radius=16, fill="#b8e3bc")
)
card.transform.position = (0.25, 0.5)


if __name__ == "__main__":
    project.render("first-render.mp4", backend="cpu", overwrite=True)
