"""Nested layers, masks, track mattes and transforms."""

import argparse
from pathlib import Path

from vestra import Interpolation, MatteMode, Project
from vestra.effects import ChromaticAberration, DirectionalBlur
from vestra.sources import Color, Rectangle, Text

SHOWCASE = Path(__file__).resolve().parents[1]


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, height = size
    unit = width / 1280
    project = Project(size=size, fps=fps, duration=8, base_directory=SHOWCASE)
    project.root.add(Color("#ebe7df"), duration=8)
    architecture = project.root.group(duration=8, z=1, id="architecture")
    for index in range(5):
        slat = architecture.add(
            Rectangle(width=85 * unit, height=520 * unit, fill="#213c4b"), duration=8
        )
        slat.transform.position = (0.30 + index * 0.10, 0.5)
        slat.transform.rotation_degrees = -18
    architecture.transform.scale.keyframe(0, 0.9)
    architecture.transform.scale.keyframe(
        8, 1.2, interpolation=Interpolation.EASE_IN_OUT
    )
    architecture.transform.rotation_degrees.keyframe(0, -5)
    architecture.transform.rotation_degrees.keyframe(8, 8)
    blur = architecture.effects.add(DirectionalBlur(radius=0, angle_degrees=18))
    blur.radius.keyframe(0, 2 * unit)
    blur.radius.keyframe(1, 0)
    architecture.effects.add(ChromaticAberration(amount=0.6 * unit, angle_degrees=0))

    window = project.root.add(
        Rectangle(
            width=width * 0.72,
            height=height * 0.58,
            fill="#ffffff",
            corner_radius=6 * unit,
        ),
        duration=8,
        visible=False,
        id="window",
    )
    window.transform.scale.keyframe(0, (0.2, 1))
    window.transform.scale.keyframe(
        1.4, (1, 1), interpolation=Interpolation.EASE_IN_OUT
    )
    window.transform.rotation_degrees.keyframe(0, -3)
    window.transform.rotation_degrees.keyframe(8, 3)
    architecture.set_matte(window, mode=MatteMode.ALPHA)
    stripe = project.root.add(
        Rectangle(width=width, height=6 * unit, fill="#ac5f41"), duration=8, z=2
    )
    stripe.transform.position = (0.5, 0.73)
    stripe.masks.add(Rectangle(width=width * 0.72, height=height, fill="#ffffff"))
    title = project.root.add(
        Text(
            "Masks + groups",
            font="assets/Manrope.ttf",
            font_size=28 * unit,
            fill="#213c4b",
        ),
        duration=8,
        z=3,
    )
    title.transform.position = (0.5, 0.13)
    footer = project.root.add(
        Text(
            "Nested layers · masks · track matte",
            font="assets/Manrope.ttf",
            font_size=22 * unit,
            fill="#626764",
        ),
        duration=8,
        z=3,
    )
    footer.transform.position = (0.5, 0.87)
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
