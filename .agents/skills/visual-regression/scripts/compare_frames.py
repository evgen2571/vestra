#!/usr/bin/env python3
"""Compare two decoded frames and emit RGBA error metrics and an amplified diff."""

import argparse
import json
import math
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def compare(reference: Path, candidate: Path, output: Path, tolerance: int) -> dict:
    with Image.open(reference) as src, Image.open(candidate) as dst:
        original = src.convert("RGBA")
        rendered = dst.convert("RGBA")
    if original.size != rendered.size:
        raise ValueError(f"frame sizes differ: {original.size} != {rendered.size}")
    if original.width == 0 or original.height == 0:
        raise ValueError("empty frame")

    diff = ImageChops.difference(original, rendered)
    stats = ImageStat.Stat(diff)
    mae = sum(stats.mean) / 4
    mse = sum(value * value for value in stats.rms) / 4
    max_error = max(high for _, high in diff.getextrema())
    mismatch = Image.new("L", original.size, 0)
    for channel in diff.split():
        mismatch = ImageChops.lighter(
            mismatch, channel.point(lambda value: 255 if value > tolerance else 0)
        )
    mismatched = mismatch.histogram()[255]

    output.mkdir(parents=True, exist_ok=True)
    # Force an opaque output: raw RGBA differences may have alpha=0 and look blank.
    bands = diff.split()
    amplified = [
        ImageChops.lighter(band, bands[3]).point(lambda value: min(value * 4, 255))
        for band in bands[:3]
    ]
    enhanced = Image.merge("RGB", amplified)
    enhanced.save(output / "difference.png")
    report = {
        "reference": str(reference),
        "candidate": str(candidate),
        "width": original.width,
        "height": original.height,
        "channels": "RGBA",
        "mae": mae,
        "max_abs_error": max_error,
        "psnr_db": None if mse == 0 else round(10 * math.log10(255 * 255 / mse), 6),
        "mismatched_pixels": mismatched,
        "mismatch_fraction": mismatched / (original.width * original.height),
        "pixel_tolerance": tolerance,
        "difference_image": str(output / "difference.png"),
    }
    (output / "metrics.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="reference PNG or other Pillow image")
    parser.add_argument("candidate", type=Path, help="candidate frame")
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--pixel-tolerance", type=int, default=0, help="0-255 per channel")
    parser.add_argument("--max-mae", type=float, help="optional gate; mean absolute RGBA error")
    parser.add_argument("--max-mismatch-fraction", type=float, help="optional gate; range 0..1")
    args = parser.parse_args()
    if not 0 <= args.pixel_tolerance <= 255:
        parser.error("--pixel-tolerance must be between 0 and 255")
    if args.max_mae is not None and (not math.isfinite(args.max_mae) or not 0 <= args.max_mae <= 255):
        parser.error("--max-mae must be finite and between 0 and 255")
    if args.max_mismatch_fraction is not None and (
        not math.isfinite(args.max_mismatch_fraction) or not 0 <= args.max_mismatch_fraction <= 1
    ):
        parser.error("--max-mismatch-fraction must be finite and between 0 and 1")
    try:
        report = compare(args.reference, args.candidate, args.output_dir, args.pixel_tolerance)
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    print(json.dumps(report, indent=2))
    failed = (args.max_mae is not None and report["mae"] > args.max_mae) or (
        args.max_mismatch_fraction is not None
        and report["mismatch_fraction"] > args.max_mismatch_fraction
    )
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
