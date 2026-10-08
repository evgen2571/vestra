"""Asset-free fine dithering with monochrome, cool, and warm palettes.

Run from the repository root: uv run python examples/python/high-level/13_palette_dither.py
"""

from pathlib import Path

from vestra import Project
from vestra.effects import ColorAdjust, OrderedDither, PaletteMap, PaletteMode
from vestra.sources import Color, Rectangle


ROOT = Path(__file__).resolve().parents[3]
PALETTES = (
    ("#050505", "#4e4e4e", "#adadad", "#f8f8f8"),
    ("#071827", "#27565d", "#69b49c", "#fff0c0"),
    ("#201222", "#7a3446", "#d07758", "#ffe2a1"),
)


def build_project() -> Project:
    project = Project(size=(640, 360), fps=30, duration=4)
    project.root.add(Color("#101010"), duration=4)
    for row, palette in enumerate(PALETTES):
        group = project.root.group(f"palette-{row}", duration=4)
        for column in range(64):
            tone = round(255 * column / 63)
            stripe = group.add(
                Rectangle(
                    width=10, height=100, fill=f"#{tone:02x}{tone:02x}{tone:02x}"
                ),
                duration=4,
            )
            stripe.transform.position = (
                (column * 10 + 5) / 640,
                (row * 120 + 60) / 360,
            )
        group.effects.add(ColorAdjust(0, 1, 0, 1))
        dither = group.effects.add(OrderedDither(palette, scale=1, matrix="bayer8"))
        dither.strength.keyframe(0, 0.25)
        dither.strength.keyframe(1, 1)
    project.post_effects.add(PaletteMap(mode=PaletteMode.RAINBOW, amount=0.1, period=2))
    return project


if __name__ == "__main__":
    output = ROOT / "examples/output/palette-dither.mp4"
    output.parent.mkdir(parents=True, exist_ok=True)
    build_project().render(str(output), backend="cpu", overwrite=True)
