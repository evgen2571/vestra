"""Image cutouts, geometric masks and grouped animation."""

import argparse
from pathlib import Path

from vestra import Interpolation, Project
from vestra.effects import Saturation
from vestra.sources import Circle, Color, Image, Rectangle, Text

SHOWCASE = Path(__file__).resolve().parents[1]


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, _ = size
    unit = width / 1280
    project = Project(size=size, fps=fps, duration=8, base_directory=SHOWCASE)
    project.root.add(Color("#ebe7df"), duration=8)

    def label(text, position, font_size, colour="#213c4b"):
        layer = project.root.add(
            Text(
                text, font="assets/Manrope.ttf", font_size=font_size * unit, fill=colour
            ),
            duration=8,
            z=5,
        )
        layer.transform.position = position

    label("Compositing", (0.5, 0.14), 42)
    label("One image · three masks · grouped animation", (0.5, 0.87), 25)
    for index, x in enumerate((0.22, 0.5, 0.78)):
        card = project.root.group(duration=8, z=index + 1)
        photo = card.add(Image("assets/landscape.jpg", sizing="cover"), duration=8)
        photo.effects.add(Saturation((0, 0.75, 1.2)[index]))
        photo.transform.scale.keyframe(0, 1)
        photo.transform.scale.keyframe(8, 1.3)
        photo.transform.position = (0.46 + index * 0.04, 0.5)
        if index == 0:
            mask = Circle(radius=150 * unit, fill="#ffffff")
        else:
            mask = Rectangle(
                width=300 * unit,
                height=320 * unit,
                fill="#ffffff",
                corner_radius=(35 if index == 1 else 0) * unit,
            )
        card.masks.add(mask)
        card.transform.position.keyframe(0, (x, 0.58))
        card.transform.position.keyframe(
            1, (x, 0.48), interpolation=Interpolation.EASE_OUT
        )
        card.transform.position.keyframe(
            4, (x, 0.52), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.position.keyframe(
            8, (x, 0.48), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.scale.keyframe(0, 0.8)
        card.transform.scale.keyframe(1, 1, interpolation=Interpolation.EASE_OUT)
        card.opacity.keyframe(0, 0)
        card.opacity.keyframe(0.6, 1)
        label(("Circle", "Rounded rectangle", "Rectangle")[index], (x, 0.76), 24)
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/compositing.mp4"
    )
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    project = build_project((320, 180), 6) if args.smoke else build_project()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
