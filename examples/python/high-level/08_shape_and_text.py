"""Render a shape-backed title card with the high-level API."""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.sources import Rectangle, Text


ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(160, 90), fps=12, duration=1, base_directory=ROOT)
    card = project.root.add(
        Rectangle(width=120, height=48, fill="#203050", corner_radius=8),
        duration=1,
        z=0,
    )
    card.transform.position = (0.5, 0.5)
    title = project.root.add(
        Text(
            "Vestra",
            font="tests/assets/VestraTest-Regular.ttf",
            font_size=28,
            fill="#ffffff",
            align="center",
        ),
        duration=1,
        z=1,
    )
    title.transform.position = (0.5, 0.5)
    title.transform.anchor = (0.5, 0.5)

    report = project.validate()
    if not report.is_valid:
        raise SystemExit("shape and text project validation failed")
    frame = project.prepare(backend="cpu").render_frame_seconds(0.5)
    print(frame.frame_number, len(frame.to_bytes()))


if __name__ == "__main__":
    main()
