"""Render a soft, animated geometric mask reveal."""

from __future__ import annotations

from pathlib import Path

from vestra import Image, MaskCoverageMode, Project
from vestra.sources import Ellipse, Rectangle


ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(320, 180), fps=30, duration=1, base_directory=ROOT)
    layer = project.root.add(Rectangle(width=320, height=180, fill="#e84a5f"), duration=1)
    reveal = layer.masks.add(
        Ellipse(width=180, height=120, fill="#ffffff"), feather=12, id="reveal"
    )
    reveal.transform.scale.keyframe(0, (0.15, 0.15))
    reveal.transform.scale.keyframe(1, (1.0, 1.0))
    image_luma = layer.masks.add(
        Image(str(ROOT / "examples/assets/green.png")),
        mode=MaskCoverageMode.LUMA,
        feather=4,
        id="image-luma",
    )
    image_luma.transform.scale = (0.75, 0.75)

    report = project.validate()
    if not report.is_valid:
        raise SystemExit("mask project validation failed")
    frame = project.prepare(backend="cpu").render_frame_seconds(0.5)
    print(frame.frame_number, len(frame.to_bytes()))


if __name__ == "__main__":
    main()
