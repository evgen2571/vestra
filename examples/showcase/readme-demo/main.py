"""Ten-second Vestra showcase: animation, media, compositing and audio-reactive graphics."""

from __future__ import annotations

import argparse
from pathlib import Path

from vestra import (
    BlurCrossfade,
    Crossfade,
    Interpolation,
    Point,
    Project,
)
from vestra.effects import (
    Bloom,
    ChromaticAberration,
    DirectionalBlur,
    GaussianBlur,
    Saturation,
)
from vestra.sources import (
    Circle,
    Color,
    Ellipse,
    Image,
    ParticleBlendMode,
    ParticleLifetimeStyle,
    ParticleSystem,
    Rectangle,
    RectangleEmitter,
    ScalarLifetimeStop,
    Spectrum2D,
    Spectrum2DLinearLayout,
    Text,
    Video,
)

SHOWCASE = Path(__file__).resolve().parents[1]
FONT = "assets/Manrope.ttf"
DURATION = 10.0
BACKGROUND, PAPER = "#111b22", "#edf0e9"
ACCENT, MUTED, WARM = "#b8d99c", "#5b807b", "#d9b89c"

LAYERS_START, LAYERS_DURATION = 0.0, 2.2
MEDIA_START, MEDIA_DURATION = 1.7, 2.3
EFFECTS_START, EFFECTS_DURATION = 3.5, 2.3
GROUPS_START, GROUPS_DURATION = 5.3, 2.3
AUDIO_START, AUDIO_DURATION = 7.1, 2.9
TRANSITION_DURATION = 0.5


def add_title(group, text: str, *, duration: float, unit: float) -> None:
    """Add a minimal scene title."""

    title = group.add(
        Text(
            text,
            font=FONT,
            font_size=34 * unit,
            fill=PAPER,
        ),
        duration=duration,
        z=20,
    )
    title.transform.position = (0.5, 0.14)

    title.opacity.keyframe(0.0, 0.0)
    title.opacity.keyframe(
        0.25,
        1.0,
        interpolation=Interpolation.EASE_OUT,
    )


def add_watermark(project: Project, *, unit: float) -> None:
    """Add a quiet renderer signature over the complete hero video."""
    watermark = project.root.add(
        Text("Rendered with Vestra", font=FONT, font_size=15 * unit, fill=PAPER),
        duration=DURATION,
        z=10_000,
    )
    watermark.transform.position = (0.905, 0.955)
    watermark.opacity = 0.42


def create_scene(
    project: Project, *, title: str, start: float, duration: float, unit: float
):
    """Create a scene with a shared background and title."""
    scene = project.root.group(start=start, duration=duration)
    scene.add(Color(BACKGROUND), duration=duration, z=-100)
    add_title(scene, title, duration=duration, unit=unit)
    return scene


def build_layers_scene(project: Project, *, unit: float):
    scene = create_scene(
        project,
        title="LAYERS + KEYFRAMES",
        start=LAYERS_START,
        duration=LAYERS_DURATION,
        unit=unit,
    )
    for index, colour in enumerate((ACCENT, MUTED, PAPER)):
        start = index * 0.1
        card = scene.add(
            Rectangle(
                width=220 * unit,
                height=190 * unit,
                fill=colour,
                corner_radius=12 * unit,
            ),
            start=start,
            duration=LAYERS_DURATION - start,
            z=index + 1,
        )
        x = 0.27 + index * 0.23
        card.transform.position.keyframe(0.0, (-0.18, 0.60))
        card.transform.position.keyframe(
            0.65, (x, 0.52), interpolation=Interpolation.EASE_OUT
        )
        card.transform.position.keyframe(
            1.70, (x, 0.55), interpolation=Interpolation.EASE_IN_OUT
        )
        card.transform.scale.keyframe(0.0, 0.55)
        card.transform.scale.keyframe(0.65, 1.0, interpolation=Interpolation.EASE_OUT)
        card.transform.rotation_degrees.keyframe(0.0, -24)
        card.transform.rotation_degrees.keyframe(
            1.70, 8 - index * 8, interpolation=Interpolation.EASE_IN_OUT
        )
    return scene


def build_media_scene(project: Project, *, unit: float):
    scene = create_scene(
        project,
        title="MEDIA + MASKS",
        start=MEDIA_START,
        duration=MEDIA_DURATION,
        unit=unit,
    )

    # A darkened, softened full-frame copy provides depth behind the reveal.
    backdrop = scene.add(
        Video("assets/city-1.mp4", sizing="cover"), duration=MEDIA_DURATION, z=0
    )
    backdrop.effects.add(Saturation(0.30))
    backdrop.effects.add(GaussianBlur(11 * unit))
    backdrop.opacity = 0.25
    backdrop.transform.scale.keyframe(0.0, 1.08)
    backdrop.transform.scale.keyframe(
        MEDIA_DURATION, 1.16, interpolation=Interpolation.EASE_IN_OUT
    )

    # The actual footage is revealed through one soft ellipse.
    footage = scene.add(
        Video("assets/city-1.mp4", sizing="cover"), duration=MEDIA_DURATION, z=2
    )
    footage.transform.scale.keyframe(0.0, 1.04)
    footage.transform.scale.keyframe(
        MEDIA_DURATION, 1.14, interpolation=Interpolation.EASE_IN_OUT
    )
    reveal = footage.masks.add(
        Ellipse(width=790 * unit, height=490 * unit, fill="#ffffff"), feather=28 * unit
    )
    reveal.transform.position.keyframe(0.0, (0.27, 0.55))
    reveal.transform.position.keyframe(
        0.85, (0.48, 0.52), interpolation=Interpolation.EASE_OUT
    )
    reveal.transform.position.keyframe(
        MEDIA_DURATION, (0.57, 0.50), interpolation=Interpolation.EASE_IN_OUT
    )
    reveal.transform.scale.keyframe(0.0, 0.32)
    reveal.transform.scale.keyframe(0.85, 0.92, interpolation=Interpolation.EASE_OUT)
    reveal.transform.scale.keyframe(
        MEDIA_DURATION, 1.08, interpolation=Interpolation.EASE_IN_OUT
    )

    credit = scene.add(
        Text(
            "City timelapse · Location Kenya · CC BY 3.0",
            font=FONT,
            font_size=14 * unit,
            fill=PAPER,
        ),
        duration=MEDIA_DURATION,
        z=20,
    )
    credit.transform.position = (0.5, 0.925)
    credit.opacity = 0.55
    return scene


def build_effects_scene(project: Project, *, unit: float):
    scene = create_scene(
        project,
        title="EFFECTS + COMPOSITING",
        start=EFFECTS_START,
        duration=EFFECTS_DURATION,
        unit=unit,
    )

    # The photograph can become very bright around the sky, so this scene gets
    # a subtle text shadow instead of a background plate.
    title_shadow = scene.add(
        Text(
            "EFFECTS + COMPOSITING",
            font=FONT,
            font_size=34 * unit,
            fill="#000000",
        ),
        duration=EFFECTS_DURATION,
        z=19,
    )
    title_shadow.transform.position = (0.503, 0.146)
    title_shadow.opacity = 0.58
    title_shadow.effects.add(GaussianBlur(3.5 * unit))

    title_shadow.opacity.keyframe(0.0, 0.0)
    title_shadow.opacity.keyframe(
        0.25,
        0.58,
        interpolation=Interpolation.EASE_OUT,
    )

    echo = scene.add(
        Image("assets/landscape.jpg", sizing="cover"), duration=EFFECTS_DURATION, z=1
    )
    echo.opacity, echo.transform.scale = 0.22, 1.08
    echo.transform.position.keyframe(0.0, (0.47, 0.52))
    echo.transform.position.keyframe(
        EFFECTS_DURATION, (0.53, 0.50), interpolation=Interpolation.EASE_IN_OUT
    )
    echo.effects.add(Saturation(0.25))
    echo.effects.add(GaussianBlur(8 * unit))
    image = scene.add(
        Image("assets/landscape.jpg", sizing="cover"), duration=EFFECTS_DURATION, z=2
    )
    image.transform.scale.keyframe(0.0, 1.13)
    image.transform.scale.keyframe(1.35, 1.0, interpolation=Interpolation.EASE_OUT)
    image.transform.rotation_degrees.keyframe(0.0, -2.5)
    image.transform.rotation_degrees.keyframe(
        1.35, 0.0, interpolation=Interpolation.EASE_OUT
    )
    saturation = image.effects.add(Saturation(0.35))
    saturation.amount.keyframe(0.0, 0.35)
    saturation.amount.keyframe(1.25, 1.05, interpolation=Interpolation.EASE_OUT)
    directional_blur = image.effects.add(
        DirectionalBlur(radius=9 * unit, angle_degrees=0)
    )
    directional_blur.radius.keyframe(0.0, 9 * unit)
    directional_blur.radius.keyframe(1.15, 0.0, interpolation=Interpolation.EASE_OUT)
    chromatic = image.effects.add(ChromaticAberration(amount=6 * unit, angle_degrees=0))
    chromatic.amount.keyframe(0.0, 6 * unit)
    chromatic.amount.keyframe(1.30, 0.0, interpolation=Interpolation.EASE_OUT)
    blur = image.effects.add(GaussianBlur(7 * unit))
    blur.radius.keyframe(0.0, 7 * unit)
    blur.radius.keyframe(1.0, 0.0, interpolation=Interpolation.EASE_OUT)
    return scene


def build_groups_scene(project: Project, *, unit: float):
    scene = create_scene(
        project,
        title="NESTED GROUPS",
        start=GROUPS_START,
        duration=GROUPS_DURATION,
        unit=unit,
    )
    parent = scene.group(duration=GROUPS_DURATION, z=2)
    parent.transform.position = (0.5, 0.53)
    parent.transform.scale.keyframe(0.0, 0.84)
    parent.transform.scale.keyframe(0.65, 1.0, interpolation=Interpolation.EASE_OUT)
    parent.transform.scale.keyframe(
        GROUPS_DURATION, 1.04, interpolation=Interpolation.EASE_IN_OUT
    )
    parent.transform.rotation_degrees.keyframe(0.0, -3)
    parent.transform.rotation_degrees.keyframe(
        GROUPS_DURATION, 4, interpolation=Interpolation.EASE_IN_OUT
    )

    # Parent boundary makes the relationship between the group and its children clear.
    frame = parent.add(
        Rectangle(
            width=790 * unit,
            height=360 * unit,
            fill=None,
            stroke=MUTED,
            stroke_width=2 * unit,
            corner_radius=18 * unit,
        ),
        duration=GROUPS_DURATION,
        z=0,
    )
    frame.opacity = 0.65
    positions = (0.30, 0.50, 0.70)
    colours = (ACCENT, PAPER, MUTED)
    for index, (x, colour) in enumerate(zip(positions, colours)):
        child = parent.add(
            Rectangle(
                width=175 * unit,
                height=175 * unit,
                fill=colour,
                corner_radius=14 * unit,
            ),
            duration=GROUPS_DURATION,
            z=index + 1,
        )
        child.transform.position.keyframe(0.0, (x, 0.56))
        child.transform.position.keyframe(
            0.70 + index * 0.10,
            (x, 0.48 + (index - 1) * 0.035),
            interpolation=Interpolation.EASE_OUT,
        )
        child.transform.position.keyframe(
            GROUPS_DURATION,
            (x, 0.53 - (index - 1) * 0.025),
            interpolation=Interpolation.EASE_IN_OUT,
        )
        child.transform.rotation_degrees.keyframe(0.0, -10 + index * 10)
        child.transform.rotation_degrees.keyframe(
            GROUPS_DURATION, 8 - index * 8, interpolation=Interpolation.EASE_IN_OUT
        )
        child.transform.scale.keyframe(0.0, 0.72)
        child.transform.scale.keyframe(
            0.65 + index * 0.10, 1.0, interpolation=Interpolation.EASE_OUT
        )

    caption = parent.add(
        Text(
            "child animation   →   parent transform",
            font=FONT,
            font_size=20 * unit,
            fill=PAPER,
        ),
        duration=GROUPS_DURATION,
        z=10,
    )
    caption.transform.position = (0.5, 0.80)
    caption.opacity = 0.70
    return scene


def build_audio_scene(project: Project, *, unit: float):
    scene = create_scene(
        project,
        title="AUDIO + PARTICLES",
        start=AUDIO_START,
        duration=AUDIO_DURATION,
        unit=unit,
    )
    energy = (
        project.audio.signal.rms()
        .remap(input=(0.0, 0.12), output=(0.78, 1.28))
        .clamp(0.78, 1.28)
        .envelope(0.03, 0.15)
    )

    particles = scene.add(
        ParticleSystem(
            emitter=RectangleEmitter(
                center=Point(0.5, 0.52),
                size=Point(0.98, 0.82),
            ),
            rate=36,
            seed=31,
            lifetime=2.4,
            size=0.0048,
            colour=WARM,
            opacity=0.55,
            speed=0.022,
            direction=-90,
            spread=55,
            blend_mode=ParticleBlendMode.ADDITIVE,
            lifetime_style=ParticleLifetimeStyle(
                size=(
                    ScalarLifetimeStop(0.0, 0.70),
                    ScalarLifetimeStop(0.55, 1.05),
                    ScalarLifetimeStop(1.0, 1.25),
                ),
                opacity=(
                    ScalarLifetimeStop(0.0, 0.0),
                    ScalarLifetimeStop(0.10, 0.85),
                    ScalarLifetimeStop(0.72, 0.60),
                    ScalarLifetimeStop(1.0, 0.0),
                ),
            ),
        ),
        duration=AUDIO_DURATION,
        z=1,
    )
    particles.opacity.bind(energy, operation="multiply")

    bounce = scene.add(
        Circle(
            radius=92 * unit,
            stroke=ACCENT,
            stroke_width=3 * unit,
        ),
        duration=AUDIO_DURATION,
        z=3,
    )
    bounce.transform.position = (0.5, 0.39)
    bounce.transform.scale.bind(energy, operation="multiply")
    bounce.opacity = 0.85
    bounce.effects.add(Bloom(threshold=0.38, radius=4 * unit, intensity=0.40))

    linear_spectrum = scene.add(
        Spectrum2D(
            band_count=44,
            min_hz=40,
            max_hz=4800,
            sensitivity=1100,
            min_bar_height_ratio=0.02,
            x=0.12,
            y=0.56,
            width=0.76,
            height=0.18,
            layout=Spectrum2DLinearLayout(
                anchor="bottom",
                band_mapping="forward",
            ),
            colour=ACCENT,
            bar_gap_ratio=0.48,
            attack_seconds=0.025,
            release_seconds=0.16,
        ),
        duration=AUDIO_DURATION,
        z=4,
    )
    linear_spectrum.effects.add(Bloom(threshold=0.40, radius=4 * unit, intensity=0.30))

    baseline = scene.add(
        Rectangle(
            width=973 * unit,
            height=2 * unit,
            fill="#b8d99c80",
        ),
        duration=AUDIO_DURATION,
        z=3,
    )
    baseline.transform.position = (0.5, 0.74)
    scene.opacity.keyframe(0.0, 1.0)
    scene.opacity.keyframe(2.15, 1.0)
    scene.opacity.keyframe(2.70, 0.0, interpolation=Interpolation.EASE_IN_OUT)
    scene.opacity.keyframe(AUDIO_DURATION, 0.0)
    return scene


def build_project(size: tuple[int, int] = (1280, 720), fps: int = 30) -> Project:
    width, _ = size
    unit = width / 1280
    project = Project(
        size=size,
        fps=fps,
        duration=DURATION,
        base_directory=SHOWCASE,
        output_audio=False,
    )
    project.root.add(Color(BACKGROUND), duration=DURATION, z=-1000)
    # Analyze this bundled loop for reactive graphics; rendered output stays silent.
    project.audio.track("analysis").add(
        "assets/synth.wav", trim_end=DURATION, gain=0.8, fade_in=0.15, fade_out=0.45
    )
    layers = build_layers_scene(project, unit=unit)
    media = build_media_scene(project, unit=unit)
    effects = build_effects_scene(project, unit=unit)
    groups = build_groups_scene(project, unit=unit)
    audio = build_audio_scene(project, unit=unit)
    add_watermark(project, unit=unit)
    transitions = (
        (layers, media, Crossfade(), MEDIA_START),
        (media, effects, BlurCrossfade(radius=7 * unit), EFFECTS_START),
        (effects, groups, Crossfade(), GROUPS_START),
        (groups, audio, BlurCrossfade(radius=6 * unit), AUDIO_START),
    )
    for outgoing, incoming, transition, start in transitions:
        project.root.transitions.add(
            outgoing, incoming, transition, start=start, duration=TRANSITION_DURATION
        )
    return project


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path, default=SHOWCASE.parent / "output/readme-demo.mp4"
    )
    parser.add_argument(
        "--smoke",
        action="store_true",
        help="render the full ten-second timeline at 320×180 and 6 fps",
    )
    args = parser.parse_args()
    project = build_project(size=(320, 180), fps=6) if args.smoke else build_project()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    project.render(args.output, backend="cpu", overwrite=True)
