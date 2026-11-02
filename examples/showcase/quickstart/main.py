"""The README quickstart; no external images, fonts or audio required."""

from vestra import Interpolation, Project
from vestra.sources import Color, Rectangle

project = Project(size=(640, 360), fps=30, duration=3)
project.root.add(Color("#101c24"))
card = project.root.add(
    Rectangle(width=180, height=140, corner_radius=16, fill="#b8e3bc")
)
card.transform.position.keyframe(0, (0.25, 0.5))
card.transform.position.keyframe(
    1.5, (0.75, 0.5), interpolation=Interpolation.EASE_IN_OUT
)
card.transform.position.keyframe(
    3, (0.25, 0.5), interpolation=Interpolation.EASE_IN_OUT
)


if __name__ == "__main__":
    project.render("quickstart.mp4", backend="cpu", overwrite=True)
