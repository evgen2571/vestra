"""Spectrum, audio signals, animated geometry and particles."""

import argparse
from pathlib import Path

from vestra import Project
from vestra.effects import Bloom
from vestra.sources import (
    Circle,
    Color,
    ParticleSystem,
    RectangleEmitter,
    Spectrum2D,
    Spectrum2DLinearLayout,
    Text,
)
from vestra import Point

SHOWCASE = Path(__file__).resolve().parents[1]


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, _ = size
    unit = width / 1280
    project = Project(size=size, fps=fps, duration=12, base_directory=SHOWCASE)
    project.root.add(Color("#1b161b"), duration=12)
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
        Circle(radius=135 * unit, stroke="#79574d", stroke_width=1.5 * unit),
        duration=12,
    )
    halo.transform.position = (0.5, 0.42)
    halo.transform.scale.bind(energy, operation="multiply")
    particles = project.root.add(
        ParticleSystem(
            emitter=RectangleEmitter(center=Point(0.5, 0.5), size=Point(0.9, 0.7)),
            rate=5,
            seed=19,
            lifetime=3,
            size=0.004,
            colour="#deb18d",
            opacity=0.25,
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
            band_count=40,
            min_hz=40,
            max_hz=4000,
            sensitivity=1000,
            min_bar_height_ratio=0.05,
            x=0.16,
            y=0.40,
            width=0.68,
            height=0.36,
            layout=Spectrum2DLinearLayout(anchor="center"),
            colour="#deb18d",
            bar_gap_ratio=0.5,
            attack_seconds=0.025,
            release_seconds=0.18,
        ),
        duration=12,
        z=2,
    )
    spectrum.effects.add(Bloom(threshold=0.5, radius=3 * unit, intensity=0.25))
    heading = project.root.add(
        Text(
            "Audio + spectrum",
            font="assets/Manrope.ttf",
            font_size=34 * unit,
            fill="#f1e4db",
        ),
        duration=12,
        z=3,
    )
    heading.transform.position = (0.5, 0.20)
    caption = project.root.add(
        Text(
            "Master audio → signals → graphics",
            font="assets/Manrope.ttf",
            font_size=22 * unit,
            fill="#a68b7d",
        ),
        duration=12,
        z=3,
    )
    caption.transform.position = (0.5, 0.86)
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
