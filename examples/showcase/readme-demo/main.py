"""README hero: keyframes, masked footage and audio-driven motion."""

import argparse
from pathlib import Path

from vestra import Interpolation, Project
from vestra.authoring import Quality
from vestra.effects import OrderedDither
from vestra.sources import (
    Circle,
    Color,
    Rectangle,
    Spectrum2D,
    Spectrum2DRadialLayout,
    Spectrum2DGradient,
    Text,
    Video,
)

SHOWCASE = Path(__file__).resolve().parents[1]
DURATION = 10
BACKGROUND, PAPER, MINT = "#101c24", "#f1efe5", "#b8e3bc"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    unit = size[0] / 1280
    project = Project(
        size=size,
        fps=fps,
        duration=DURATION,
        base_directory=SHOWCASE,
        quality=Quality.HIGH,
        output_audio=False,
    )
    project.root.add(Color(BACKGROUND), z=-100)
    # Keep the analysis input audible in the mix; only the exported track is disabled.
    project.audio.track("synth").add("assets/synth.wav", trim_end=DURATION)
    energy = (
        project.audio.signal.rms()
        .remap(input=(0.06, 0.14), output=(0.8, 1.25))
        .clamp(0.8, 1.25)
        .envelope(0.025, 0.12)
    )

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

    def left_label(text, y, font_size, fill=PAPER):
        layer = label(project.root, text, (0.075, y), font_size, fill)
        layer.transform.anchor = (0, 0.5)

    left_label("Vestra", 0.105, 28, MINT)
    label(project.root, "Python + JSON", (0.74, 0.105), 21)
    left_label("Video", 0.34, 92)
    left_label("composition", 0.49, 59, MINT)
    left_label("Layers, motion and sound", 0.64, 22)
    left_label("CPU / WGPU", 0.70, 18, MINT)
    label(project.root, "Rendered with Vestra · 720p / 30 fps", (0.5, 0.925), 17, MINT)

    rule = project.root.add(Rectangle(width=1120 * unit, height=unit, fill="#36504f"))
    rule.transform.position = (0.5, 0.835)
    progress = project.root.add(
        Rectangle(width=1120 * unit, height=2 * unit, fill=MINT)
    )
    progress.transform.anchor = (0, 0.5)
    progress.transform.position = (0.0625, 0.835)
    progress.transform.scale.keyframe(0, (0.001, 1))
    progress.transform.scale.keyframe(DURATION, (1, 1))

    # Fade adjacent groups over the persistent background.
    def scene(start, duration, caption):
        group = project.root.group(start=start, duration=duration)
        group.opacity.keyframe(0, 0)
        group.opacity.keyframe(0.3, 1)
        group.opacity.keyframe(duration - 0.3, 1)
        group.opacity.keyframe(duration, 0)
        label(group, caption, (0.745, 0.765), 19, MINT, duration)
        return group

    motion = scene(0, 3.5, "Keyframes + nested compositions")
    for index, colour in enumerate(("#527e79", MINT, PAPER)):
        card = motion.group(duration=3.5, z=index)
        card.add(
            Rectangle(
                width=240 * unit,
                height=290 * unit,
                corner_radius=24 * unit,
                fill=colour,
            ),
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
        inset = card.add(
            Circle(radius=45 * unit, stroke=BACKGROUND, stroke_width=2 * unit), z=1
        )
        inset.transform.position = (0.5, 0.46)
        label(card, f"0{index + 1}", (0.5, 0.46), 28, BACKGROUND, 3.5)

    media = scene(3.2, 3.6, "Video + masks + palette effects")
    clip = media.group(duration=3.6)
    footage = clip.add(Video("assets/city-2.mp4", sizing="cover"))
    footage.transform.scale = 0.46
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
    dither = clip.effects.add(
        OrderedDither(
            (BACKGROUND, "#527e79", MINT, PAPER),
            matrix="bayer8",
            scale=max(1, round(2 * unit)),
            input_exposure=-0.3,
        )
    )
    dither.amount.keyframe(0, 0)
    dither.amount.keyframe(1.5, 0)
    dither.amount.keyframe(2.7, 1, interpolation=Interpolation.EASE_IN_OUT)

    reactive = scene(6.5, 3.5, "Spectrum + audio-signal bindings")
    reactive.add(
        Spectrum2D(
            band_count=40,
            min_hz=40,
            max_hz=400,
            sensitivity=800,
            x=0.54,
            y=0.16,
            width=0.40,
            height=0.60,
            layout=Spectrum2DRadialLayout(inner_radius_ratio=0.55),
            gradient=Spectrum2DGradient(MINT, "#527e79", "across_bands"),
            colour=MINT,
            bar_gap_ratio=0.5,
            min_bar_height_ratio=0.06,
            attack_seconds=0.025,
            release_seconds=0.16,
        ),
        duration=3.5,
    )
    halo = reactive.add(
        Circle(radius=66 * unit, stroke=MINT, stroke_width=3 * unit),
        duration=3.5,
        id="audio-halo",
    )
    halo.transform.position = (0.74, 0.46)
    halo.transform.scale.bind(energy, operation="multiply")
    label(reactive, "RMS", (0.74, 0.46), 23, PAPER, 3.5)
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
