"""Compare mask coverage using procedural artwork and the CPU renderer."""

from vestra import (
    Circle,
    Group,
    MaskCoverageMode,
    MaskOperation,
    Project,
    Rectangle,
    Text,
)

project = Project(size=(640, 480), fps=30, duration=3, background="#171d22")

artwork = Group(size=(240, 140))
for index, color in enumerate(("#6488d8", "#87b3c4", "#b5d0af", "#e8bd78", "#d78176")):
    artwork.add(
        Rectangle(width=48, height=140, fill=color),
        position=((index + 0.5) / 5, 0.5),
    )

# Both alpha and luma receive exactly the same opaque mask artwork.
coverage = Group(size=(240, 140))
coverage.add(Rectangle(width=240, height=140, fill="#404040"))
coverage.add(Circle(radius=56, fill="#ffffff"))

alpha = project.root.add(artwork, position=(0.25, 0.3))
alpha.masks.add(coverage, mode=MaskCoverageMode.ALPHA)

luma = project.root.add(artwork, position=(0.75, 0.3))
luma.masks.add(coverage, mode=MaskCoverageMode.LUMA)

union = project.root.add(artwork, position=(0.25, 0.77))
union.masks.add(Circle(radius=54, position=(0.35, 0.5)))
union.masks.add(
    Circle(radius=54, position=(0.65, 0.5)),
    operation=MaskOperation.UNION,
)

subtract = project.root.add(artwork, position=(0.75, 0.77))
subtract.masks.add(Circle(radius=60))
subtract.masks.add(Circle(radius=30), operation=MaskOperation.SUBTRACT)

for label, position in (
    ("Alpha", (0.25, 0.075)),
    ("Luma", (0.75, 0.075)),
    ("Union", (0.25, 0.545)),
    ("Subtract", (0.75, 0.545)),
):
    project.root.add(Text(label, font_size=30), position=position)

assert project.validate().is_valid
project.render("concepts-masks.mp4", backend="cpu", overwrite=True)
