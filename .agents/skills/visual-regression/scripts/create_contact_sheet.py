#!/usr/bin/env python3
"""Make a labeled contact sheet of rendered frames for human review."""

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageOps


def create_sheet(images: list[Path], labels: list[str], output: Path, columns: int, width: int, height: int) -> None:
    margin, label_height = 12, 32
    rows = (len(images) + columns - 1) // columns
    sheet = Image.new(
        "RGB",
        (columns * (width + margin) + margin, rows * (height + label_height + margin) + margin),
        "#202020",
    )
    draw = ImageDraw.Draw(sheet)
    for index, (path, label) in enumerate(zip(images, labels, strict=True)):
        with Image.open(path) as image:
            thumbnail = ImageOps.contain(image.convert("RGB"), (width, height), Image.Resampling.LANCZOS)
        x = margin + (index % columns) * (width + margin)
        y = margin + (index // columns) * (height + label_height + margin)
        sheet.paste(thumbnail, (x + (width - thumbnail.width) // 2, y + (height - thumbnail.height) // 2))
        draw.text((x + 4, y + height + 6), label[:90], fill="white")
    output.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(output)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("images", type=Path, nargs="+")
    parser.add_argument("--labels", nargs="*", help="one label per input (default: file names)")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--columns", type=int, default=3)
    parser.add_argument("--tile-width", type=int, default=400)
    parser.add_argument("--tile-height", type=int, default=225)
    args = parser.parse_args()
    if args.columns < 1 or args.tile_width < 1 or args.tile_height < 1:
        parser.error("columns and tile dimensions must be positive")
    if args.labels is not None and len(args.labels) != len(args.images):
        parser.error("--labels must have exactly one value per image")
    try:
        create_sheet(
            args.images,
            args.labels if args.labels is not None else [p.name for p in args.images],
            args.output,
            args.columns,
            args.tile_width,
            args.tile_height,
        )
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    print(args.output)


if __name__ == "__main__":
    main()
