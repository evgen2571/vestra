"""Render a rectangle with a geometric ellipse mask."""

from __future__ import annotations

from pathlib import Path

from vestra import MaskOperation, Project
from vestra.sources import Ellipse, Rectangle


ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(320, 180), fps=30, duration=1, base_directory=ROOT)
    layer = project.root.add(Rectangle(width=320, height=180, fill="#e84a5f"), duration=1)
    layer.masks.add(Ellipse(width=180, height=120, fill="#ffffff"), id="ellipse")
    layer.masks.add(
        Ellipse(width=64, height=48, fill="#ffffff"),
        operation=MaskOperation.SUBTRACT,
        id="hole",
    )

    report = project.validate()
    if not report.is_valid:
        raise SystemExit("mask project validation failed")
    frame = project.prepare(backend="cpu").render_frame_seconds(0.5)
    print(frame.frame_number, len(frame.to_bytes()))


if __name__ == "__main__":
    main()
