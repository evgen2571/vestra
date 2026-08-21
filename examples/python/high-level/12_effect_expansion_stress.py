"""Exercise the expanded effects with transforms, masks, mattes, and groups."""

from __future__ import annotations

from pathlib import Path

from vestra import Image, MatteMode, MaskOperation, Project
from vestra.effects import ChromaticAberration, DirectionalBlur, MotionTile, RadialBlur
from vestra.sources import Ellipse

ROOT = Path(__file__).resolve().parents[3]


def main() -> None:
    project = Project(size=(320, 180), fps=24, duration=2, base_directory=ROOT)
    content = project.root.add(
        Image(str(ROOT / "examples/assets/red.png")),
        duration=2,
        id="tiled-content",
    )
    tile = content.effects.add(
        MotionTile(output_width_percent=200, output_height_percent=180)
    )
    tile.output_width_percent.keyframe(1.0, 260)
    content.transform.scale.keyframe(0.0, (1.0, 1.0))
    content.transform.scale.keyframe(2.0, (1.35, 1.2))
    content.transform.rotation_degrees.keyframe(0.0, -8)
    content.transform.rotation_degrees.keyframe(2.0, 12)
    content.effects.add(DirectionalBlur(radius=12, angle_degrees=35))
    content.effects.add(ChromaticAberration(amount=4, angle_degrees=0))
    content.masks.add(
        Ellipse(width=270, height=150, fill="#ffffff"),
        operation=MaskOperation.INTERSECT,
        feather=10,
        id="soft-window",
    )

    matte = project.root.add(
        Ellipse(width=260, height=140, fill="#ffffff"),
        duration=2,
        id="content-matte",
        visible=False,
        z=3,
    )
    content.set_matte(matte, mode=MatteMode.ALPHA)

    group = project.root.group("radial-group", duration=2, z=1)
    grouped = group.add(Image(str(ROOT / "examples/assets/blue.png")), duration=2)
    grouped.effects.add(RadialBlur(amount=10, center=(0.45, 0.55)))

    report = project.validate()
    if not report.is_valid:
        raise SystemExit("effect expansion stress project failed validation")
    frame = project.prepare(backend="cpu").render_frame_seconds(1.0)
    print(frame.frame_number, len(frame.to_bytes()))


if __name__ == "__main__":
    main()
