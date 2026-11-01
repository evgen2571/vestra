"""README hero: keyframed cards, masked footage and animated palette dithering."""

import argparse
from pathlib import Path

from vestra import Interpolation, Project
from vestra.effects import OrderedDither
from vestra.sources import Circle, Color, Image, Rectangle, Text, Video

SHOWCASE = Path(__file__).resolve().parents[1]
DURATION = 10
BACKGROUND, PAPER, MINT = "#101c24", "#f1efe5", "#b8e3bc"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    unit = size[0] / 1280
    project = Project(size=size, fps=fps, duration=DURATION, base_directory=SHOWCASE)
    project.root.add(Color(BACKGROUND), z=-100)

    def label(parent, text, position, font_size, fill=PAPER, duration=DURATION):
        layer = parent.add(
            Text(
                text, font="assets/Manrope.ttf", font_size=font_size * unit, fill=fill
            ),
            duration=duration,
            z=20,
        )
        layer.transform.position = position
        return layer

    label(project.root, "V E S T R A", (0.16, 0.105), 26, MINT)
    label(project.root, "PYTHON  →  VIDEO", (0.79, 0.105), 18)
    label(project.root, "Make it", (0.245, 0.33), 100)
    label(project.root, "move.", (0.245, 0.49), 112, MINT)
    label(project.root, "Your ideas. Your timeline. Your code.", (0.252, 0.655), 21)
    label(project.root, "Every frame rendered with Vestra", (0.5, 0.925), 17, MINT)

    rule = project.root.add(Rectangle(width=1120 * unit, height=unit, fill="#36504f"))
    rule.transform.position = (0.5, 0.835)
    progress = project.root.add(
        Rectangle(width=1120 * unit, height=2 * unit, fill=MINT)
    )
    progress.transform.anchor = (0, 0.5)
    progress.transform.position = (0.0625, 0.835)
    progress.transform.scale.keyframe(0, (0.001, 1))
    progress.transform.scale.keyframe(DURATION, (1, 1))

    # Overlapping opacity ramps keep the hero alive through every scene change.
    def scene(start, duration, caption):
        group = project.root.group(start=start, duration=duration)
        if start:
            group.opacity.keyframe(0, 0)
            group.opacity.keyframe(0.3, 1)
        group.opacity.keyframe(duration - 0.3, 1)
        group.opacity.keyframe(duration, 0)
        label(group, caption, (0.745, 0.765), 19, MINT, duration)
        return group

    motion = scene(0, 3.5, "01 / KEYFRAMES + LAYERS")
    for index, colour in enumerate(("#527e79", MINT, PAPER)):
        card = motion.add(
            Rectangle(
                width=240 * unit,
                height=290 * unit,
                corner_radius=24 * unit,
                fill=colour,
            ),
            z=index,
        )
        x = 0.67 + index * 0.07
        angle = -20 + index * 20
        card.transform.position.keyframe(0, (x, 0.47))
        card.transform.position.keyframe(
            1.7, (x, 0.42 + index * 0.04), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.position.keyframe(
            3.5, (x, 0.47), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.rotation_degrees.keyframe(0, angle)
        card.transform.rotation_degrees.keyframe(
            1.7, -angle, interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.rotation_degrees.keyframe(
            3.5, angle, interpolation=Interpolation.EASE_IN_OUT
        )
        dot = motion.add(Circle(radius=10 * unit, fill=colour), z=5)
        dot.transform.position = (0.64 + index * 0.10, 0.17)

    media = scene(3.2, 3.6, "02 / FOOTAGE + MASKS")
    clip = media.add(Video("assets/city-1.mp4", sizing="cover"))
    clip.masks.add(
        Rectangle(
            width=530 * unit, height=330 * unit, corner_radius=32 * unit, fill="#ffffff"
        )
    )
    clip.transform.position = (0.74, 0.455)
    clip.transform.scale = 0.96
    clip.transform.rotation_degrees.keyframe(0, -4)
    clip.transform.rotation_degrees.keyframe(
        3.6, 4, interpolation=Interpolation.EASE_IN_OUT
    )
    label(media, "Location Kenya · CC BY 3.0", (0.74, 0.70), 15, PAPER, 3.6)

    effects = scene(6.5, 3.5, "03 / PALETTES + DITHER")
    photo = effects.add(Image("assets/landscape.jpg", sizing="cover"))
    photo.masks.add(
        Rectangle(
            width=530 * unit, height=330 * unit, corner_radius=32 * unit, fill="#ffffff"
        )
    )
    photo.transform.position = (0.74, 0.455)
    photo.transform.rotation_degrees = -3
    dither = photo.effects.add(
        OrderedDither(
            (BACKGROUND, "#527e79", MINT, PAPER),
            matrix="bayer8",
            scale=max(1, round(3 * unit)),
            input_gamma=1.25,
        )
    )
    dither.amount.keyframe(0, 0)
    dither.amount.keyframe(1.2, 1, interpolation=Interpolation.EASE_IN_OUT)
    dither.phase.keyframe(1.2, 0)
    dither.phase.keyframe(3.5, 0.45)
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/readme-demo.mp4"
    )
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    project = build_project((320, 180), 6) if args.smoke else build_project()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
