"""Spectrum, audio signals, animated geometry and particles."""

import argparse
from pathlib import Path

from vestra import Project
from vestra.authoring import Quality
from vestra.effects import Bloom
from vestra.sources import (
    Circle,
    Color,
    ParticleSystem,
    RectangleEmitter,
    Spectrum2D,
    Spectrum2DRadialLayout,
    Spectrum2DGradient,
    Text,
)
from vestra import Point

SHOWCASE = Path(__file__).resolve().parents[1]


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, _ = size
    unit = width / 1280
    project = Project(
        size=size, fps=fps, duration=12, base_directory=SHOWCASE, quality=Quality.HIGH
    )
    project.root.add(Color("#141222"), duration=12)
    project.audio.track("synth").add(
        "assets/synth.wav", trim_end=12, gain=0.85, fade_in=0.4, fade_out=1
    )
    energy = (
        project.audio.signal.rms()
        .remap(input=(0, 0.18), output=(0.85, 1.08))
        .clamp(0.85, 1.08)
    )
    energy = energy.envelope(0.03, 0.2)
    halo = project.root.add(
        Circle(radius=145 * unit, stroke="#71e5e0", stroke_width=2 * unit),
        duration=12,
    )
    halo.transform.position = (0.5, 0.5)
    halo.transform.scale.bind(energy, operation="multiply")
    particles = project.root.add(
        ParticleSystem(
            emitter=RectangleEmitter(center=Point(0.5, 0.5), size=Point(0.9, 0.7)),
            rate=22,
            seed=19,
            lifetime=3,
            size=0.004,
            colour="#bba5eb",
            opacity=0.45,
            speed=0.025,
            direction=-90,
            spread=25,
        ),
        duration=12,
        z=1,
    )
    particles.opacity.bind(energy, operation="multiply")
    spectrum = project.root.add(
        Spectrum2D(
            band_count=48,
            min_hz=40,
            max_hz=1600,
            sensitivity=800,
            min_bar_height_ratio=0.08,
            x=0.24,
            y=0.13,
            width=0.52,
            height=0.74,
            layout=Spectrum2DRadialLayout(inner_radius_ratio=0.62),
            gradient=Spectrum2DGradient("#71e5e0", "#ff8fa4", "across_bands"),
            colour="#bba5eb",
            bar_gap_ratio=0.5,
            attack_seconds=0.025,
            release_seconds=0.18,
        ),
        duration=12,
        z=2,
    )
    spectrum.effects.add(Bloom(threshold=0.65, radius=4 * unit, intensity=0.25))
    heading = project.root.add(
        Text(
            "Spectrum",
            font="assets/Manrope.ttf",
            font_size=45 * unit,
            fill="#f1efe5",
        ),
        duration=12,
        z=3,
    )
    heading.transform.position = (0.5, 0.5)
    caption = project.root.add(
        Text(
            "Audio-reactive spectrum and particles",
            font="assets/Manrope.ttf",
            font_size=20 * unit,
            fill="#bba5eb",
        ),
        duration=12,
        z=3,
    )
    caption.transform.position = (0.5, 0.91)
    # Fade the visual scene together, including Spectrum2D's adapter presentation.
    for layer in project.root.layers[1:]:
        layer.opacity.keyframe(0, 0)
        layer.opacity.keyframe(0.6, 1)
        layer.opacity.keyframe(11, 1)
        layer.opacity.keyframe(12, 0)
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/audio-visualizer.mp4"
    )
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    project = build_project((320, 180), 6) if args.smoke else build_project()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
