"""Video clips, reframing, transitions, titles and silent output."""

import argparse
from pathlib import Path

from vestra import Interpolation, Project
from vestra.effects import Saturation, Vignette
from vestra.sources import Rectangle, Text, Video

SHOWCASE = Path(__file__).resolve().parents[1]
FONT = "assets/Manrope.ttf"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, _ = size
    unit = width / 1280
    project = Project(
        size=size, fps=fps, duration=14, base_directory=SHOWCASE, output_audio=False
    )
    shots = []
    for number, start in enumerate((0, 4.5, 9), 1):
        shot = project.root.add(
            Video(f"assets/city-{number}.mp4", sizing="cover"),
            start=start,
            duration=5,
            z=number,
            id=f"shot-{number}",
        )
        shot.transform.scale.keyframe(0, 1.02)
        shot.transform.scale.keyframe(5, 1.10)
        shot.effects.add(Saturation(0.65))
        shot.effects.add(Vignette(0.25, 1.0, 0.7, "#000000"))
        shots.append(shot)
    # Fade each incoming shot over the opaque outgoing shot. This keeps the
    # dissolve fully covered, without a dip to black from two fading layers.
    for shot in shots[1:]:
        shot.opacity.keyframe(0, 0)
        shot.opacity.keyframe(0.5, 1)

    # Title and rule move together as a small nested overlay.
    title = project.root.group(start=0.6, duration=3.6, z=5, id="title")
    title.add(
        Text("Video editing", font=FONT, font_size=56 * unit, fill="#f0ece4"),
        duration=3.6,
    )
    title.transform.position = (0.5, 0.78)
    title.opacity.keyframe(0, 0)
    title.opacity.keyframe(0.5, 1, interpolation=Interpolation.EASE_OUT)
    title.opacity.keyframe(3.0, 1)
    title.opacity.keyframe(3.6, 0)
    rule = title.add(
        Rectangle(width=190 * unit, height=2 * unit, fill="#c9ac76"), duration=3.6, z=1
    )
    rule.transform.position = (0.5, 0.57)
    credit = project.root.add(
        Text(
            "Footage: Location Kenya · CC BY 3.0",
            font=FONT,
            font_size=20 * unit,
            fill="#f0ece4",
        ),
        start=11.5,
        duration=2.5,
        z=6,
    )
    credit.transform.position = (0.5, 0.91)
    # Finish gently instead of cutting off a moving shot.
    for shot in shots:
        if shot is shots[0]:
            shot.opacity.keyframe(0, 0)
            shot.opacity.keyframe(0.6, 1)
        if shot is shots[-1]:
            shot.opacity.keyframe(4.2, 1)
            shot.opacity.keyframe(5, 0)
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/real-media-edit.mp4"
    )
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    project = build_project((320, 180), 6) if args.smoke else build_project()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
