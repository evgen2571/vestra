"""Image cutouts, geometric masks and grouped animation."""

import argparse
from pathlib import Path

from vestra import Interpolation, Project
from vestra.authoring import Quality
from vestra.effects import Saturation
from vestra.sources import Circle, Color, Image, Rectangle, Text

SHOWCASE = Path(__file__).resolve().parents[1]
ASSETS = SHOWCASE.parents[1] / "assets"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    # Keep pixel-sized masks and type proportional in the smoke render.
    resolution_scale = size[0] / 1280
    project = Project(
        size=size, fps=fps, duration=8, base_directory=ASSETS, quality=Quality.HIGH
    )
    project.root.add(Color("#ebe7df"), duration=8)

    def label(text, position, font_size, colour="#213c4b"):
        layer = project.root.add(
            Text(
                text,
                font="fonts/Manrope.ttf",
                font_size=font_size * resolution_scale,
                fill=colour,
            ),
            duration=8,
            z=5,
        )
        layer.transform.position = position

    label("Compositing", (0.5, 0.14), 42)
    label("One image · three masks · grouped animation", (0.5, 0.87), 25)

    cutouts = (
        {
            "title": "Circle",
            "mask": Circle(radius=150 * resolution_scale, fill="#ffffff"),
            "saturation": 0.0,
            "image_x": 0.46,
            "card_x": 0.22,
        },
        {
            "title": "Rounded rectangle",
            "mask": Rectangle(
                width=300 * resolution_scale,
                height=320 * resolution_scale,
                fill="#ffffff",
                corner_radius=35 * resolution_scale,
            ),
            "saturation": 0.75,
            "image_x": 0.5,
            "card_x": 0.5,
        },
        {
            "title": "Rectangle",
            "mask": Rectangle(
                width=300 * resolution_scale,
                height=320 * resolution_scale,
                fill="#ffffff",
            ),
            "saturation": 1.2,
            "image_x": 0.54,
            "card_x": 0.78,
        },
    )
    for index, cutout in enumerate(cutouts):
        card_x = cutout["card_x"]
        card = project.root.group(duration=8, z=index + 1)
        photo = card.add(Image("images/landscape.jpg", sizing="cover"), duration=8)

        photo.effects.add(Saturation(amount=cutout["saturation"]))

        # Zoom the image inside a fixed mask; moving the card moves both.
        photo.transform.position = (cutout["image_x"], 0.5)
        photo.transform.scale.keyframe(0, 1)
        photo.transform.scale.keyframe(8, 1.3)
        card.masks.add(cutout["mask"])

        # Keyframe times are local to this eight-second card.
        card.transform.position.keyframe(0, (card_x, 0.58))
        card.transform.position.keyframe(
            1, (card_x, 0.48), interpolation=Interpolation.EASE_OUT
        )
        card.transform.position.keyframe(
            4, (card_x, 0.52), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.position.keyframe(
            8, (card_x, 0.48), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.scale.keyframe(0, 0.8)
        card.transform.scale.keyframe(1, 1, interpolation=Interpolation.EASE_OUT)
        card.opacity.keyframe(0, 0)
        card.opacity.keyframe(0.6, 1)
        card.opacity.keyframe(7.4, 1)
        card.opacity.keyframe(8, 0)
        label(cutout["title"], (card_x, 0.76), font_size=24)
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
