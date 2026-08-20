"""Combine root transitions, an image preset, a flash, and a post-effect."""

from __future__ import annotations

from pathlib import Path

from vestra import Flash, Preset, Project
from vestra.effects import ColorAdjust
from vestra.sources import Image
from vestra.transitions import Crossfade

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(160, 90), fps=12, duration=2, base_directory=ROOT)
    first = project.root.add(
        Image("examples/assets/red.png", sizing="cover"), duration=1
    )
    second = project.root.add(
        Image("examples/assets/blue.png", sizing="cover"), start=0.5, duration=1.5
    )
    project.root.transitions.add(first, second, Crossfade(), start=0.5, duration=0.5)
    first.presets.add(Preset("slow_drift", intensity=0.5, duration=1.0))
    project.flashes.add(Flash(1.0, 0.1, "#ffffff", opacity=0.5, fade_out=0.1))
    project.post_effects.add(ColorAdjust(0.0, 1.0, 0.0, 1.0))
    print(len(project.snapshot().to_dict()["visual"]["transitions"]))


if __name__ == "__main__":
    main()
