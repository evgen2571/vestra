#!/usr/bin/env python3
"""Render the downloadable documentation examples and refresh their web previews.

Requires the source-checkout Python environment (just python-sync) and FFmpeg.
The website build itself never invokes this script or imports Vestra.
"""

import argparse
import hashlib
import json
import runpy
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target/docs-media"
ASSETS = ROOT / "docs/assets/renders"
EXAMPLES = (
    ("first_render", "first-render", (640, 360), 0.5),
    ("first_animation", "quickstart", (640, 360), 0.75),
    ("keyframes", "keyframes", (960, 540), 1.3),
)


def verify_video(
    path: Path, size: tuple[int, int], *, seconds=3, fps=30, audio=False
) -> None:
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
    video = next(
        stream for stream in info["streams"] if stream["codec_type"] == "video"
    )
    sound = [stream for stream in info["streams"] if stream["codec_type"] == "audio"]
    assert len(info["streams"]) == 1 + int(audio), path
    assert bool(sound) == audio, path
    if audio:
        assert sound[0]["codec_name"] == "aac", path
        assert abs(float(sound[0]["duration"]) - seconds) < 0.05, path
    assert video["codec_name"] == "h264", path
    assert video["pix_fmt"] == "yuv420p", path
    assert (video["width"], video["height"]) == size, path
    assert video["avg_frame_rate"] == f"{fps}/1", path
    assert int(video["nb_read_frames"]) == round(seconds * fps), path
    assert abs(float(info["format"]["duration"]) - seconds) < 0.05, path
    timestamps = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_frames",
            "-show_entries",
            "frame=best_effort_timestamp_time",
            "-of",
            "json",
            str(path),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    frames = json.loads(timestamps.stdout)["frames"]
    assert len(frames) == round(seconds * fps), path
    assert all(
        abs(float(frame["best_effort_timestamp_time"]) - index / fps) < 0.0001
        for index, frame in enumerate(frames)
    ), path
    print(
        f"Verified {path.name}: H.264, {size[0]}×{size[1]}, "
        f"{len(frames)} frames, {seconds} seconds, {'AAC' if audio else 'silent'}"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--learning", action="store_true", help="Render the visual learning studies"
    )
    args = parser.parse_args()
    if args.learning:
        render_learning()
        return
    OUTPUT.mkdir(parents=True, exist_ok=True)
    ASSETS.mkdir(parents=True, exist_ok=True)
    for source_name, output_name, size, poster_time in EXAMPLES:
        asset_name = source_name.replace("_", "-")
        subprocess.run(
            [sys.executable, str(ROOT / f"docs/assets/examples/{source_name}.py")],
            cwd=OUTPUT,
            check=True,
        )
        original = OUTPUT / f"{output_name}.mp4"
        verify_video(original, size)
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
                str(ASSETS / f"{asset_name}.webp"),
            ],
            check=True,
        )
        preview = ASSETS / f"{asset_name}.mp4"
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
        verify_video(preview, size)


def web_video(original, name, size, seconds, poster_time):
    """Create compact controls-ready delivery media and verify both versions."""
    verify_video(original, size, seconds=seconds)
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-i",
            str(original),
            "-c:v",
            "libx264",
            "-crf",
            "26",
            "-preset",
            "slow",
            "-pix_fmt",
            "yuv420p",
            "-an",
            "-map_metadata",
            "-1",
            "-movflags",
            "+faststart",
            str(ASSETS / f"{name}.mp4"),
        ],
        check=True,
    )
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
            "88",
            "-map_metadata",
            "-1",
            str(ASSETS / f"{name}.webp"),
        ],
        check=True,
    )
    verify_video(ASSETS / f"{name}.mp4", size, seconds=seconds)
    return {
        "source_sha256": hashlib.sha256(original.read_bytes()).hexdigest(),
        "poster_seconds": poster_time,
        "size": size,
        "seconds": seconds,
        "fps": 30,
        "backend": "cpu",
        "audio": False,
        "web_sha256": hashlib.sha256((ASSETS / f"{name}.mp4").read_bytes()).hexdigest(),
        "poster_sha256": hashlib.sha256(
            (ASSETS / f"{name}.webp").read_bytes()
        ).hexdigest(),
    }


def render_learning():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    ASSETS.mkdir(parents=True, exist_ok=True)
    programs = (
        "effects_catalog",
        "concepts_masks",
        "transition_comparisons",
        "audio_response",
        "group_placements",
    )
    manifest = {
        "program_sha256": {
            name: hashlib.sha256(
                (ROOT / f"docs/assets/examples/{name}.py").read_bytes()
            ).hexdigest()
            for name in programs
        },
        "input_sha256": {
            name: hashlib.sha256(
                (ROOT / f"docs/assets/examples/{name}").read_bytes()
            ).hexdigest()
            for name in ("landscape.jpg", "learning-inputs.txt")
        },
        "stills": {},
    }
    motion = (
        "camera-shake",
        "motion-blur",
        "motion-tile",
        "crt",
    )
    effects = runpy.run_path(str(ROOT / "docs/assets/examples/effects_catalog.py"))
    for name in [*effects["effect_cases"](), "order"]:
        if name in motion or name == "original":
            continue
        project = effects["comparison_project"](name)
        image_path = ASSETS / f"effect-{name}.webp"
        effects["save_still"](project, image_path)
        manifest["stills"][f"effect-{name}"] = {
            "seconds": 0.7,
            "backend": "cpu",
            "web_sha256": hashlib.sha256(image_path.read_bytes()).hexdigest(),
        }
    groups = runpy.run_path(str(ROOT / "docs/assets/examples/group_placements.py"))
    image_path = ASSETS / "group-placements.webp"
    effects["save_still"](groups["project"], image_path)
    manifest["stills"]["group-placements"] = {
        "seconds": 0.7,
        "backend": "cpu",
        "web_sha256": hashlib.sha256(image_path.read_bytes()).hexdigest(),
    }
    for name in motion:
        original = OUTPUT / f"effect-{name}.mp4"
        result = effects["comparison_project"](name).render(
            original, backend="cpu", overwrite=True
        )
        assert result.selected_backend == "cpu"
        manifest[f"effect-{name}"] = web_video(
            original, f"effect-{name}", (640, 520), 3, 0.7
        )
    for module in ("concepts_masks", "transition_comparisons", "audio_response"):
        subprocess.run(
            [sys.executable, str(ROOT / f"docs/assets/examples/{module}.py")],
            cwd=OUTPUT,
            check=True,
        )
    for name in ("crossfade", "push", "zoom", "blur"):
        key = f"transition-{name}"
        manifest[key] = web_video(OUTPUT / f"{key}.mp4", key, (640, 360), 3, 1.5)
    for name in ("bounded", "smoothing", "spectra"):
        key = f"audio-{name}"
        manifest[key] = web_video(OUTPUT / f"{key}.mp4", key, (640, 560), 4, 1.2)
    manifest["concepts-masks"] = web_video(
        OUTPUT / "concepts-masks.mp4", "concepts-masks", (640, 480), 3, 0.5
    )
    (ASSETS / "learning-media.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
