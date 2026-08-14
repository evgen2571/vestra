"""Prepare once and request random-access frames from the native runtime."""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.sources import Color

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(16, 10), fps=10, duration=2, base_directory=ROOT)
    project.root.add(Color("#204060"), duration=2)
    prepared = project.prepare(backend="cpu")
    for seconds in (1.25, 0.25):
        frame = prepared.render_frame_seconds(seconds)
        print(frame.frame_number, frame.timestamp_seconds, len(frame.to_bytes()))


if __name__ == "__main__":
    main()
