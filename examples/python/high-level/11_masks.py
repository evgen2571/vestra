"""Render a soft, animated geometric mask reveal.

Run from the repository root: uv run python examples/python/high-level/11_masks.py
"""

from __future__ import annotations

from pathlib import Path

from vestra import Image, MaskCoverageMode, Project
from vestra.sources import Ellipse, Rectangle


ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(320, 180), fps=30, duration=1, base_directory=ROOT)
    card = project.root.add(
        Rectangle(width=320, height=180, fill="#e84a5f"), duration=1
    )
    window = Ellipse(width=180, height=120, fill="#ffffff")
    reveal = card.masks.add(window, feather=12, id="reveal")

    reveal.transform.scale.keyframe(0, 0.15)
    reveal.transform.scale.keyframe(1, 1)

    # Luma multiplies the reveal by the image brightness; green is partly transparent.
    brightness_mask = card.masks.add(
        Image(ROOT / "examples/assets/green.png"),
        mode=MaskCoverageMode.LUMA,
        feather=4,
        id="image-luma",
    )
    brightness_mask.transform.scale = (0.75, 0.75)

    report = project.validate()
    if not report.is_valid:
        raise SystemExit("mask project validation failed")
    prepared = project.prepare(backend="cpu")
    frame = prepared.render_frame_seconds(0.5)
    print(f"Frame {frame.frame_number}: {len(frame.to_bytes())} RGBA bytes")


if __name__ == "__main__":
    main()
