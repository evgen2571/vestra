"""Ten seconds of animated layers, media, masks, effects, groups and audio."""

import argparse
from pathlib import Path

from vestra import Crossfade, DirectionalPush, Interpolation, Project
from vestra.effects import GaussianBlur, Saturation
from vestra.sources import Circle, Color, Image, Rectangle, Spectrum2D, Text, Video

SHOWCASE = Path(__file__).resolve().parents[1]
FONT = "assets/Manrope.ttf"
INK, PAPER, ACCENT = "#111b22", "#edf0e9", "#b8d99c"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, height = size
    unit = width / 1280
    project = Project(size=size, fps=fps, duration=10, base_directory=SHOWCASE)
    project.root.add(Color(INK), duration=10)
    project.audio.track("audio").add(
        "assets/rapid2-excerpt.mp3", trim_end=10, gain=0.45, fade_in=0.08, fade_out=0.5
    )

    def label(group, text, position, duration, font_size=44):
        layer = group.add(
            Text(text, font=FONT, font_size=font_size * unit, fill=PAPER),
            duration=duration,
            z=10,
        )
        layer.transform.position = position
        return layer

    def scene(title, start, duration):
        group = project.root.group(start=start, duration=duration)
        group.add(Color(INK), duration=duration)
        label(group, title, (0.5, 0.16), duration)
        return group

    layers = scene("Layers + keyframes", 0, 2.2)
    for index, colour in enumerate((ACCENT, "#5b807b", PAPER)):
        tile = layers.add(
            Rectangle(
                width=230 * unit, height=200 * unit, fill=colour, corner_radius=8 * unit
            ),
            start=index * 0.12,
            duration=2.2 - index * 0.12,
            z=index + 1,
        )
        x = 0.26 + index * 0.24
        tile.transform.position.keyframe(0, (-0.15, 0.6))
        tile.transform.position.keyframe(
            0.6, (x, 0.52), interpolation=Interpolation.EASE_OUT
        )
        tile.transform.position.keyframe(1.714286, (x, 0.56))
        tile.transform.rotation_degrees.keyframe(0, -25)
        tile.transform.rotation_degrees.keyframe(1.714286, 8 - index * 8)
        tile.transform.scale.keyframe(0, 0.6)
        tile.transform.scale.keyframe(0.6, 1)
    label(layers, "Position · scale · rotation", (0.5, 0.84), 2.2, 28)

    media = scene("Video + masks", 1.714286, 2.2)
    video = media.add(Video("assets/city-1.mp4", sizing="cover"), duration=2.2, z=1)
    video.transform.scale.keyframe(0, 1)
    video.transform.scale.keyframe(2.2, 1.2)
    window = video.masks.add(
        Rectangle(
            width=width * 0.76,
            height=height * 0.52,
            fill="#ffffff",
            corner_radius=8 * unit,
        )
    )
    window.transform.scale.keyframe(0, (0.02, 1))
    window.transform.scale.keyframe(0.65, (1, 1), interpolation=Interpolation.EASE_OUT)
    label(media, "Animated mask · reframing", (0.5, 0.84), 2.2, 28)
    label(media, "Footage: Location Kenya · CC BY 3.0", (0.5, 0.94), 2.2, 18)

    effects = scene("Image effects", 3.428572, 2.2)
    for index, x in enumerate((0.29, 0.71)):
        photo = effects.add(
            Image("assets/landscape.jpg", sizing="cover"), duration=2.2, z=1
        )
        photo.transform.position = (x, 0.52)
        photo.transform.scale = 0.36
        saturation = photo.effects.add(Saturation(0))
        if index:
            saturation.amount.keyframe(0, 0)
            saturation.amount.keyframe(1.4, 1.2)
            blur = photo.effects.add(GaussianBlur(14 * unit))
            blur.radius.keyframe(0, 14 * unit)
            blur.radius.keyframe(1.4, 0)
        label(
            effects, "Grayscale" if index == 0 else "Blur + colour", (x, 0.8), 2.2, 28
        )

    groups = scene("Groups + transitions", 5.142858, 2.2)
    nested = groups.group(duration=2.2, z=1)
    for index, colour in enumerate((ACCENT, "#5b807b", PAPER)):
        tile = nested.add(
            Rectangle(width=160 * unit, height=160 * unit, fill=colour),
            duration=2.2,
            z=index,
        )
        tile.transform.position = (0.3 + index * 0.2, 0.53)
        tile.transform.rotation_degrees.keyframe(0, 0)
        tile.transform.rotation_degrees.keyframe(2.2, 90)
    nested.transform.rotation_degrees.keyframe(0, -10)
    nested.transform.rotation_degrees.keyframe(2.2, 10)
    nested.transform.scale.keyframe(0, 0.8)
    nested.transform.scale.keyframe(2.2, 1.1)
    label(groups, "Nested layers · push · crossfade", (0.5, 0.84), 2.2, 28)

    audio = scene("Audio + spectrum", 6.857144, 2.4)
    pulse = (
        project.audio.signal.rms()
        .remap(input=(0, 0.12), output=(0.8, 1.3))
        .envelope(0.03, 0.12)
    )
    circle = audio.add(
        Circle(radius=80 * unit, stroke=ACCENT, stroke_width=3 * unit),
        duration=2.4,
        z=1,
    )
    circle.transform.position = (0.5, 0.40)
    circle.transform.scale.bind(pulse, operation="multiply")
    audio.add(
        Spectrum2D(
            band_count=32,
            min_hz=40,
            max_hz=1500,
            sensitivity=1400,
            x=0.17,
            y=0.48,
            width=0.66,
            height=0.24,
            colour=ACCENT,
            min_bar_height_ratio=0.02,
            bar_gap_ratio=0.4,
        ),
        duration=2.4,
        z=2,
    )
    label(audio, "Audio signals → graphics", (0.5, 0.84), 2.4, 28)

    for outgoing, incoming, start in (
        (layers, media, 1.714286),
        (media, effects, 3.428572),
        (effects, groups, 5.142858),
        (groups, audio, 6.857144),
    ):
        transition = (
            DirectionalPush(angle_degrees=180) if start == 5.142858 else Crossfade()
        )
        project.root.transitions.add(
            outgoing, incoming, transition, start=start, duration=0.4
        )

    final = scene("Vestra", 8.914286, 1.085714)
    label(final, "Layers · masks · effects · audio", (0.5, 0.52), 1.085714, 38)
    label(final, "Video output", (0.5, 0.72), 1.085714, 28)
    label(final, "Music: Rapid2 · PeriTune · CC BY 4.0", (0.5, 0.94), 1.085714, 18)
    final.opacity.keyframe(0, 0)
    final.opacity.keyframe(0.25, 1)
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/readme-demo.mp4"
    )
    parser.add_argument(
        "--smoke", action="store_true", help="full timeline at 320×180, 6 fps"
    )
    args = parser.parse_args()
    project = build_project((320, 180), 6) if args.smoke else build_project()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
