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
ASSETS = SHOWCASE.parents[1] / "assets"
DURATION = 10
FOOTAGE = SHOWCASE.parents[1] / "assets/footage"

BACKGROUND, PAPER, MINT = "#101c24", "#f1efe5", "#b8e3bc"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    # Keep pixel-sized geometry proportional in the smaller smoke render.
    resolution_scale = size[0] / 1280
    project = Project(
        size=size,
        fps=fps,
        duration=DURATION,
        base_directory=ASSETS,
        quality=Quality.HIGH,
        output_audio=False,
    )
    project.root.add(Color(BACKGROUND), z=-100)
    # Keep the analysis input audible in the mix; only the exported track is disabled.
    project.audio.track("synth").add("audio/synth.wav", trim_end=DURATION)
    # Bound the RMS multiplier before smoothing so beats expand the ring gently.
    loudness = project.audio.signal.rms()
    scale_response = loudness.remap(input=(0.06, 0.14), output=(0.8, 1.25))
    bounded_scale = scale_response.clamp(0.8, 1.25)
    smoothed_scale = bounded_scale.envelope(0.025, 0.12)

    def label(parent, text, position, font_size, fill=PAPER, duration=DURATION):
        layer = parent.add(
            Text(
                text,
                font="fonts/Manrope.ttf",
                font_size=font_size * resolution_scale,
                fill=fill,
            ),
            duration=duration,
            z=20,
        )
        layer.transform.position = position
        return layer

    def left_label(text, y, font_size, fill=PAPER):
        layer = label(project.root, text, (0.075, y), font_size=font_size, fill=fill)
        layer.transform.anchor = (0, 0.5)

    # Persistent labels and the progress rule sit outside the three scenes.
    left_label("Vestra", 0.105, 28, MINT)
    label(project.root, "Python + JSON", (0.74, 0.105), font_size=21)
    left_label("Video", 0.34, 92)
    left_label("composition", 0.49, 59, MINT)
    left_label("Layers, motion and sound", 0.64, 22)
    left_label("CPU / WGPU", 0.70, 18, MINT)
    label(
        project.root,
        "Rendered with Vestra · 720p / 30 fps",
        (0.5, 0.925),
        font_size=17,
        fill=MINT,
    )

    rule = project.root.add(
        Rectangle(
            width=1120 * resolution_scale, height=resolution_scale, fill="#36504f"
        )
    )
    rule.transform.position = (0.5, 0.835)
    progress = project.root.add(
        Rectangle(width=1120 * resolution_scale, height=2 * resolution_scale, fill=MINT)
    )
    progress.transform.anchor = (0, 0.5)
    progress.transform.position = (0.0625, 0.835)
    progress.transform.scale.keyframe(0, (0.001, 1))
    progress.transform.scale.keyframe(DURATION, (1, 1))

    # Scene keyframes start at zero, even when the scene starts later in the project.
    def scene(start, duration, caption):
        group = project.root.group(start=start, duration=duration)
        group.opacity.keyframe(0, 0)
        group.opacity.keyframe(0.3, 1)
        group.opacity.keyframe(duration - 0.3, 1)
        group.opacity.keyframe(duration, 0)
        label(
            group, caption, (0.745, 0.765), font_size=19, fill=MINT, duration=duration
        )
        return group

    motion = scene(0, 3.5, "Keyframes + nested compositions")
    cards = (
        ("#527e79", 0.67, 0.42, -20),
        (MINT, 0.74, 0.46, 0),
        (PAPER, 0.81, 0.50, 20),
    )
    for index, (colour, card_x, raised_y, angle) in enumerate(cards):
        card = motion.group(duration=3.5, z=index)
        card.add(
            Rectangle(
                width=240 * resolution_scale,
                height=290 * resolution_scale,
                corner_radius=24 * resolution_scale,
                fill=colour,
            ),
        )
        card.transform.position.keyframe(0, (card_x, 0.47))
        card.transform.position.keyframe(
            1.7, (card_x, raised_y), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.position.keyframe(
            3.5, (card_x, 0.47), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.rotation_degrees.keyframe(0, angle)
        card.transform.rotation_degrees.keyframe(
            1.7, -angle, interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.rotation_degrees.keyframe(
            3.5, angle, interpolation=Interpolation.EASE_IN_OUT
        )
        inset = card.add(
            Circle(
                radius=45 * resolution_scale,
                stroke=BACKGROUND,
                stroke_width=2 * resolution_scale,
            ),
            z=1,
        )
        inset.transform.position = (0.5, 0.46)
        label(
            card,
            f"0{index + 1}",
            (0.5, 0.46),
            font_size=28,
            fill=BACKGROUND,
            duration=3.5,
        )

    media = scene(3.2, 3.6, "Video + masks + palette effects")
    clip = media.group(duration=3.6)
    footage = clip.add(Video(str(FOOTAGE / "valley.mp4"), sizing="cover"))
    # The footage zoom and the group mask are independent; the group rotates both.
    footage.transform.scale = 0.46
    window = Rectangle(
        width=530 * resolution_scale,
        height=330 * resolution_scale,
        corner_radius=32 * resolution_scale,
        fill="#ffffff",
    )
    clip.masks.add(window)
    clip.transform.position = (0.74, 0.455)
    clip.transform.scale = 0.96
    clip.transform.rotation_degrees.keyframe(0, -4)
    clip.transform.rotation_degrees.keyframe(
        3.6, 4, interpolation=Interpolation.EASE_IN_OUT
    )

    # Introduce dithering only after the footage has become visible.
    dither = clip.effects.add(
        OrderedDither(
            palette=(BACKGROUND, "#527e79", MINT, PAPER),
            matrix="bayer8",
            scale=max(1, round(2 * resolution_scale)),
            input_exposure=-0.3,
        )
    )
    dither.amount.keyframe(0, 0)
    dither.amount.keyframe(1.5, 0)
    dither.amount.keyframe(2.7, 1, interpolation=Interpolation.EASE_IN_OUT)

    # Spectrum analysis follows project audio time, including this delayed scene.
    reactive = scene(6.5, 3.5, "Spectrum + audio-signal bindings")
    spectrum_source = Spectrum2D(
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
    )
    reactive.add(spectrum_source, duration=3.5)
    halo = reactive.add(
        Circle(
            radius=66 * resolution_scale, stroke=MINT, stroke_width=3 * resolution_scale
        ),
        duration=3.5,
        id="audio-halo",
    )
    halo.transform.position = (0.74, 0.46)
    halo.transform.scale.bind(smoothed_scale, operation="multiply")
    label(reactive, "RMS", (0.74, 0.46), font_size=23, fill=PAPER, duration=3.5)
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
