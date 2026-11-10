"""One photograph, three animated treatments: source, halftone and ASCII."""

import argparse
from pathlib import Path

from vestra import Interpolation, Project
from vestra.authoring import Quality
from vestra.effects import Ascii, Halftone
from vestra.sources import Color, Image, Rectangle, Text

SHOWCASE = Path(__file__).resolve().parents[1]
ASSETS = SHOWCASE.parents[1] / "assets"
DURATION = 6


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    # Keep pixel-sized geometry proportional in the smaller smoke render.
    resolution_scale = size[0] / 1280
    project = Project(
        size=size,
        fps=fps,
        duration=DURATION,
        base_directory=ASSETS,
        quality=Quality.HIGH,
    )
    project.root.add(Color("#101c24"))

    def label(text, position, font_size, colour="#f1efe5"):
        layer = project.root.add(
            Text(
                text,
                font="fonts/Manrope.ttf",
                font_size=font_size * resolution_scale,
                fill=colour,
            ),
            z=10,
        )
        layer.transform.position = position

    label("Photo, halftone and ASCII", (0.5, 0.11), 36)
    halftone = Halftone(
        cell_size=max(2, 7 * resolution_scale),
        foreground="#ffb08c",
        background="#301f35",
    )
    ascii_letters = Ascii(
        cell_width=max(2, 7 * resolution_scale),
        cell_height=max(3, 10 * resolution_scale),
        foreground="#b8e3bc",
        background="#101c24",
        mode="fill",
        source_mix=0.05,
    )

    # Each panel uses the same crop and motion, so only its treatment changes.
    treatments = (
        ("Original", 0.19, None),
        ("Halftone", 0.50, halftone),
        ("ASCII", 0.81, ascii_letters),
    )
    for name, panel_x, effect in treatments:
        group = project.root.group(duration=DURATION)
        photo = group.add(Image("images/landscape.jpg", sizing="cover"))
        if effect is not None:
            photo.effects.add(effect)

        photo.transform.scale.keyframe(0, 1)
        photo.transform.scale.keyframe(3, 1.06, interpolation=Interpolation.EASE_IN_OUT)
        photo.transform.scale.keyframe(6, 1, interpolation=Interpolation.EASE_IN_OUT)

        # Mask the group, not the image, to keep the frame fixed during the zoom.
        group.masks.add(
            Rectangle(
                width=360 * resolution_scale,
                height=430 * resolution_scale,
                fill="#ffffff",
                corner_radius=20 * resolution_scale,
            )
        )
        group.transform.position = (panel_x, 0.51)
        label(name, (panel_x, 0.87), 20, "#b8e3bc")
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/lookbook.mp4"
    )
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    project = build_project((320, 180), 6) if args.smoke else build_project()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
