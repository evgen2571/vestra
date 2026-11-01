"""The README quickstart; no external images, fonts or audio required."""

from vestra import Interpolation, Project
from vestra.sources import Color, Rectangle

project = Project(size=(640, 360), fps=30, duration=3)
project.root.add(Color("#101c24"))
ease = Interpolation.EASE_IN_OUT

for index, colour in enumerate(("#527e79", "#b8e3bc", "#f1efe5")):
    card = project.root.add(
        Rectangle(width=140, height=180, corner_radius=16, fill=colour)
    )
    card.transform.position = (0.35 + index * 0.15, 0.5)
    angle = (index - 1) * 25
    card.transform.rotation_degrees.keyframe(0, angle)
    card.transform.rotation_degrees.keyframe(1.5, -angle, interpolation=ease)
    card.transform.rotation_degrees.keyframe(3, angle, interpolation=ease)

if __name__ == "__main__":
    project.render("quickstart.mp4", backend="cpu", overwrite=True)
