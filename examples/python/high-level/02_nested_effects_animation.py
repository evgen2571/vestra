"""Use a nested composition, layer effects, and typed animation handles."""

from __future__ import annotations

from pathlib import Path

from vestra import CubicBezier, Interpolation, Project
from vestra.effects import Bloom, Brightness
from vestra.sources import Color

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(160, 90), fps=12, duration=2, base_directory=ROOT)
    scene = project.root
    group = scene.group("title-card", start=0.25, duration=1.5, z=1)
    card = group.add(Color("#202040"), duration=1.5)
    card.opacity.keyframe(0.0, 0.0)
    card.opacity.keyframe(0.5, 1.0, interpolation=Interpolation.EASE_OUT)
    card.transform.scale = 0.9
    card.transform.scale.keyframe(
        0.0, 0.9, interpolation=CubicBezier(0.25, 0.1, 0.25, 1.0)
    )
    card.transform.scale.keyframe(1.5, 1.0)
    card.effects.add(Brightness(0.1))
    card.effects.add(Bloom(threshold=0.5, radius=2.0, intensity=0.3))
    report = project.validate()
    if not report.is_valid:
        raise SystemExit("nested composition project validation failed")
    snapshot = project.snapshot()
    print(snapshot.to_dict()["visual"]["clips"][0]["source"]["clips"][0]["id"])


if __name__ == "__main__":
    main()
