"""Spectrum, audio signals, animated geometry and particles."""

import argparse
from pathlib import Path

from vestra import Point, Project
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

SHOWCASE = Path(__file__).resolve().parents[1]
ASSETS = SHOWCASE.parents[1] / "assets"


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    # Keep pixel-sized geometry proportional in the smaller smoke render.
    resolution_scale = size[0] / 1280
    project = Project(
        size=size, fps=fps, duration=12, base_directory=ASSETS, quality=Quality.HIGH
    )
    project.root.add(Color("#141222"), duration=12)
    project.audio.track("synth").add(
        "audio/synth.wav", trim_end=12, gain=0.85, fade_in=0.4, fade_out=1
    )

    # Map this recording's RMS range to a gentle multiplier, then bound peaks.
    loudness = project.audio.signal.rms()
    scale_response = loudness.remap(input=(0, 0.18), output=(0.85, 1.08))
    bounded_scale = scale_response.clamp(0.85, 1.08)
    # A fast attack follows beats; a slower release avoids jitter between them.
    smoothed_scale = bounded_scale.envelope(0.03, 0.2)

    halo = project.root.add(
        Circle(
            radius=145 * resolution_scale,
            stroke="#71e5e0",
            stroke_width=2 * resolution_scale,
        ),
        duration=12,
    )
    halo.transform.position = (0.5, 0.5)
    halo.transform.scale.bind(smoothed_scale, operation="multiply")

    particle_source = ParticleSystem(
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
    )
    particles = project.root.add(
        particle_source,
        duration=12,
        z=1,
    )
    particles.opacity.bind(smoothed_scale, operation="multiply")

    # Frequency bands have their own attack/release smoothing, separate from RMS.
    spectrum_source = Spectrum2D(
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
    )
    spectrum = project.root.add(
        spectrum_source,
        duration=12,
        z=2,
    )
    spectrum.effects.add(
        Bloom(threshold=0.65, radius=4 * resolution_scale, intensity=0.25)
    )

    heading = project.root.add(
        Text(
            "Spectrum",
            font="fonts/Manrope.ttf",
            font_size=45 * resolution_scale,
            fill="#f1efe5",
        ),
        duration=12,
        z=3,
    )
    heading.transform.position = (0.5, 0.5)
    caption = project.root.add(
        Text(
            "Audio-reactive spectrum and particles",
            font="fonts/Manrope.ttf",
            font_size=20 * resolution_scale,
            fill="#bba5eb",
        ),
        duration=12,
        z=3,
    )
    caption.transform.position = (0.5, 0.91)
    # Fade every foreground layer together while the background stays opaque.
    for layer in (halo, particles, spectrum, heading, caption):
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
