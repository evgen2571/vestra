"""A ten-second composition: geometry, a landscape reveal, then a title card."""

from pathlib import Path
import argparse

from vestra import Crossfade, Interpolation, Project
from vestra.effects import Saturation
from vestra.sources import Color, Image, Rectangle, Text

SHOWCASE = Path(__file__).resolve().parents[1]
FONT = "assets/Manrope.ttf"
INK, PAPER, ACCENT = "#111b22", "#edf0e9", "#b8d99c"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, height = size
    unit = width / 1280
    project = Project(size=size, fps=fps, duration=10, base_directory=SHOWCASE)
    project.root.add(Color(INK), duration=10, id="background")

    # The opening has one accent and room for the title to breathe.
    line = project.root.add(
        Rectangle(width=4 * unit, height=150 * unit, fill=ACCENT),
        duration=2.4,
        z=1,
        id="opening-line",
    )
    line.transform.position = (0.21, 0.48)
    line.transform.scale.keyframe(0, (1, 0.05))
    line.transform.scale.keyframe(0.8, (1, 1), interpolation=Interpolation.EASE_OUT)
    title = project.root.add(
        Text("Vestra", font=FONT, font_size=96 * unit, fill=PAPER),
        start=0.3,
        duration=2.1,
        z=2,
        id="opening-title",
    )
    title.transform.position = (0.43, 0.48)
    title.opacity.keyframe(0, 0)
    title.opacity.keyframe(0.6, 1, interpolation=Interpolation.EASE_OUT)

    # An owned mask reveals real media; the image drifts inside its window.
    landscape = project.root.group(start=1.8, duration=5.7, z=3, id="landscape-scene")
    photo = landscape.add(Image("assets/landscape.jpg", sizing="cover"), duration=5.7)
    photo.transform.scale.keyframe(0, 1.08)
    photo.transform.scale.keyframe(5.7, 1.18)
    photo.effects.add(Saturation(0.8))
    window = landscape.masks.add(
        Rectangle(
            width=width * 0.84,
            height=height * 0.70,
            fill="#ffffff",
            corner_radius=8 * unit,
        ),
        id="window",
    )
    window.transform.scale.keyframe(0, (1, 0.01))
    window.transform.scale.keyframe(1, (1, 1), interpolation=Interpolation.EASE_IN_OUT)
    label = landscape.add(
        Text(
            "A timeline. A composition. A frame.",
            font=FONT,
            font_size=26 * unit,
            fill=PAPER,
        ),
        start=1.2,
        duration=4.5,
        z=1,
    )
    label.transform.position = (0.5, 0.76)
    label.opacity.keyframe(0, 0)
    label.opacity.keyframe(0.5, 1)

    # Nested geometry replaces the photograph through an ordinary crossfade.
    geometry = project.root.group(start=5.8, duration=2.7, z=4, id="geometry-scene")
    geometry.add(Color(INK), duration=2.7)
    for index, colour in enumerate((ACCENT, "#5b807b", PAPER)):
        panel = geometry.add(
            Rectangle(
                width=180 * unit, height=280 * unit, fill=colour, corner_radius=8 * unit
            ),
            duration=2.7,
            z=index + 1,
        )
        panel.transform.position = (0.32 + index * 0.18, 0.5)
        panel.transform.rotation_degrees.keyframe(0, -12 + index * 12)
        panel.transform.rotation_degrees.keyframe(
            2.7, 0, interpolation=Interpolation.EASE_IN_OUT
        )
        panel.transform.scale.keyframe(0, 0.8)
        panel.transform.scale.keyframe(2.7, 1)
    project.root.transitions.add(
        landscape, geometry, Crossfade(), start=5.8, duration=0.8
    )
    geometry.opacity.keyframe(1.9, 1)
    geometry.opacity.keyframe(2.7, 0)

    final = project.root.group(start=7.8, duration=2.2, z=5, id="final-card")
    wordmark = final.add(
        Text("VESTRA", font=FONT, font_size=84 * unit, fill=PAPER), duration=2.2
    )
    wordmark.transform.position = (0.5, 0.44)
    subtitle = final.add(
        Text(
            "Programmatic video rendering", font=FONT, font_size=28 * unit, fill=ACCENT
        ),
        duration=2.2,
        z=1,
    )
    subtitle.transform.position = (0.5, 0.59)
    final.opacity.keyframe(0, 0)
    final.opacity.keyframe(0.6, 1, interpolation=Interpolation.EASE_OUT)
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/readme-demo.mp4"
    )
    parser.add_argument(
        "--smoke",
        action="store_true",
        help="render the full timeline at 320×180, 6 fps",
    )
    args = parser.parse_args()
    project = build_project((320, 180), 6) if args.smoke else build_project()
    report = project.validate()
    if not report.is_valid:
        raise SystemExit(str(report.errors))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
