"""Render four controlled transitions. Run: python transition_comparisons.py"""

from vestra import (
    BlurCrossfade,
    Circle,
    CompositionLayer,
    Crossfade,
    Interpolation,
    Project,
    PushLeft,
    Rectangle,
    Text,
    TransitionDefinition,
    ZoomCrossfade,
)


def add_scene(scene: CompositionLayer, incoming: bool) -> None:
    """Use the same two original scenes for every transition family."""
    background, accent = ("#34324f", "#f2b184") if incoming else ("#193b43", "#9edbc7")
    scene.add(Rectangle(width=640, height=360, fill=background))
    scene.add(
        Text("B" if incoming else "A", font_size=108, fill=accent),
        position=(0.25, 0.53),
    )
    scene.add(
        Text("Incoming" if incoming else "Outgoing", font_size=24, fill="#f0f0e8"),
        position=(0.25, 0.79),
    )
    scene.add(Circle(radius=64, stroke=accent, stroke_width=7), position=(0.70, 0.48))
    for index in range(5):
        scene.add(
            Rectangle(width=112, height=7, fill=accent),
            position=(0.70, 0.31 + index * 0.085),
            rotation=25 if incoming else -25,
        )


def definitions() -> tuple[tuple[str, str, TransitionDefinition], ...]:
    # Linear progress makes the four spatial/opacity treatments comparable.
    return (
        ("crossfade", "Crossfade", Crossfade(easing=Interpolation.LINEAR)),
        (
            "push",
            "Push left / distance 1",
            PushLeft(distance=1, easing=Interpolation.LINEAR),
        ),
        (
            "zoom",
            "Zoom crossfade / 1.3, 0.7",
            ZoomCrossfade(
                outgoing_zoom=1.3, incoming_start_zoom=0.7, easing=Interpolation.LINEAR
            ),
        ),
        (
            "blur",
            "Blur crossfade / 16 px",
            BlurCrossfade(radius=16, easing=Interpolation.LINEAR),
        ),
    )


def build_project(definition: TransitionDefinition, label: str) -> Project:
    project = Project(size=(640, 360), fps=30, duration=3, background="#101c24")
    outgoing = project.root.group("outgoing scene", start=0, duration=2, z=0)
    incoming = project.root.group("incoming scene", start=1, duration=2, z=1)
    add_scene(outgoing, incoming=False)
    add_scene(incoming, incoming=True)

    # The parent owns a transition between the two whole scene placements.
    project.root.transitions.add(outgoing, incoming, definition, start=1, duration=1)
    project.root.add(
        Text(label, font_size=22, fill="#f0f0e8"),
        position=(0.5, 0.10),
        z=2,
    )
    return project


def main() -> None:
    for name, label, definition in definitions():
        project = build_project(definition, label)
        result = project.render(f"transition-{name}.mp4", backend="cpu", overwrite=True)
        print(
            f"Rendered {result.output_path}; selected backend: {result.selected_backend}"
        )


if __name__ == "__main__":
    main()
