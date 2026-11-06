#!/usr/bin/env python3
"""Render the downloadable documentation examples and refresh their web previews.

Requires the source-checkout Python environment (just python-sync) and FFmpeg.
The website build itself never invokes this script or imports Vestra.
"""

import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target/docs-media"
ASSETS = ROOT / "docs/assets/renders"
EXAMPLES = (
    ("home_card", "home-card", 1.5),
    ("first_render", "first-render", 1.5),
    ("first_animation", "first-animation", 1.5),
    ("keyframes", "keyframes", 1.3),
)


def verify_video(path: Path) -> None:
    """Fail before publishing a preview if encoding lost frames or timing."""
    result = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-count_frames",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
            str(path),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    info = json.loads(result.stdout)
    video = info["streams"][0]
    assert len(info["streams"]) == 1, path
    assert video["codec_name"] == "h264", path
    assert (video["width"], video["height"]) == (960, 540), path
    assert video["avg_frame_rate"] == "30/1", path
    assert int(video["nb_read_frames"]) == 90, path
    assert abs(float(info["format"]["duration"]) - 3) < 0.05, path
    print(f"Verified {path.name}: H.264, 960×540, 90 frames, 3 seconds, silent")


def main() -> None:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    ASSETS.mkdir(parents=True, exist_ok=True)
    for source_name, output_name, poster_time in EXAMPLES:
        subprocess.run(
            [sys.executable, str(ROOT / f"docs/assets/examples/{source_name}.py")],
            cwd=OUTPUT,
            check=True,
        )
        original = OUTPUT / f"{output_name}.mp4"
        verify_video(original)
        subprocess.run(
            [
                "ffmpeg",
                "-v",
                "error",
                "-y",
                "-ss",
                str(poster_time),
                "-i",
                str(original),
                "-frames:v",
                "1",
                "-c:v",
                "libwebp",
                "-quality",
                "85",
                "-map_metadata",
                "-1",
                str(ASSETS / f"{output_name}.webp"),
            ],
            check=True,
        )
        if source_name in {"first_animation", "keyframes"}:
            preview = ASSETS / f"{output_name}.mp4"
            subprocess.run(
                [
                    "ffmpeg",
                    "-v",
                    "error",
                    "-y",
                    "-i",
                    str(original),
                    "-an",
                    "-c:v",
                    "libx264",
                    "-crf",
                    "26",
                    "-preset",
                    "slow",
                    "-pix_fmt",
                    "yuv420p",
                    "-movflags",
                    "+faststart",
                    "-map_metadata",
                    "-1",
                    str(preview),
                ],
                check=True,
            )
            verify_video(preview)


if __name__ == "__main__":
    main()
