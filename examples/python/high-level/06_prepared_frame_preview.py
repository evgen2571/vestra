"""Prepare once and request random-access frames from the native runtime.

Run from the repository root: uv run python examples/python/high-level/06_prepared_frame_preview.py
"""

from __future__ import annotations

from pathlib import Path

from vestra import Project
from vestra.sources import Color

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(16, 10), fps=10, duration=2, base_directory=ROOT)
    project.root.add(Color("#204060"), duration=2)

    # Request frames out of order without preparing the project again.
    prepared = project.prepare(backend="cpu")
    for seconds in (1.25, 0.25):
        frame = prepared.render_frame_seconds(seconds)
        print(
            f"Frame {frame.frame_number} at {frame.timestamp_seconds}s: {len(frame.to_bytes())} RGBA bytes"
        )


if __name__ == "__main__":
    main()
