"""Combine root transitions, an image preset, a flash, and a post-effect.

Run from the repository root: uv run python examples/python/high-level/05_transition_flash_preset_post_effect.py
"""

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

    # The two images overlap from project second 0.5 to second 1.
    project.root.transitions.add(first, second, Crossfade(), start=0.5, duration=0.5)

    first.presets.add(Preset("slow_drift", intensity=0.5, duration=1.0))
    flash = Flash(start=1.0, duration=0.1, colour="#ffffff", opacity=0.5, fade_out=0.1)
    project.flashes.add(flash)
    project.post_effects.add(
        ColorAdjust(exposure=0.0, gamma=1.0, black_point=0.0, white_point=1.0)
    )
    project.snapshot()
    print("Configured one crossfade, a slow drift, a flash and a post-effect")


if __name__ == "__main__":
    main()
