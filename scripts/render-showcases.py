#!/usr/bin/env python3
"""Render and probe the public showcases; offline smoke uses explicit test footage."""

import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SHOWCASE = ROOT / "examples/showcase"
SCENES = {
    "readme-demo": 10,
    "real-media-edit": 14,
    "audio-visualizer": 12,
    "compositing": 8,
}


def load(path: Path):
    spec = importlib.util.spec_from_file_location(path.parent.name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(*args: str) -> None:
    subprocess.run(args, check=True)


def prepare_offline(directory: Path) -> None:
    """Keep fixture inputs separate from the real, licensed showcase footage."""
    assets = directory / "assets"
    assets.mkdir()
    for name in ("Manrope.ttf", "landscape.jpg"):
        shutil.copyfile(SHOWCASE / "assets" / name, assets / name)
    load(SHOWCASE / "prepare-assets.py").write_audio(assets / "synth.wav")
    for number in range(1, 4):
        run(
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            f"testsrc2=size=320x180:rate=6:duration=5, hue=h={number * 40}",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
            str(assets / f"city-{number}.mp4"),
        )


def probe(
    output: Path, size: tuple[int, int], fps: int, duration: int, audio: bool
) -> None:
    result = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
            str(output),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    info = json.loads(result.stdout)
    video = next(
        stream for stream in info["streams"] if stream["codec_type"] == "video"
    )
    assert (video["width"], video["height"]) == size, output
    assert video["avg_frame_rate"] == f"{fps}/1", output
    assert int(video["nb_frames"]) == duration * fps, output
    assert abs(float(info["format"]["duration"]) - duration) < 0.1, output
    audio_streams = [
        stream for stream in info["streams"] if stream["codec_type"] == "audio"
    ]
    assert bool(audio_streams) == audio, output
    if audio:
        assert audio_streams[0]["codec_name"] == "aac", output
        assert audio_streams[0]["sample_rate"] == "48000", output
    print(
        f"Verified {output.name}: {size[0]}×{size[1]}, {duration * fps} frames, audio={audio}"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--smoke", action="store_true", help="full timelines at 320×180 / 6 fps"
    )
    parser.add_argument(
        "--offline",
        action="store_true",
        help="smoke only: use generated video fixtures, no download",
    )
    parser.add_argument("--output-dir", type=Path, default=ROOT / "examples/output")
    parser.add_argument(
        "--preview",
        action="store_true",
        help="regenerate the README GIF from the full hero render",
    )
    parser.add_argument(
        "--update-results",
        action="store_true",
        help="replace committed MP4s/posters with full renders",
    )
    args = parser.parse_args()
    if args.offline and not args.smoke:
        parser.error(
            "--offline requires --smoke; full showcases use the documented real assets"
        )
    if (args.preview or args.update_results) and args.smoke:
        parser.error("--preview and --update-results require a full render")
    size, fps = ((320, 180), 6) if args.smoke else ((1280, 720), 30)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="vestra-showcase-") as temporary:
        base = Path(temporary) if args.offline else SHOWCASE
        if args.offline:
            print(
                "OFFLINE SMOKE: generated test footage, not the public real-media showcase"
            )
            prepare_offline(base)
        for name, duration in SCENES.items():
            module = load(SHOWCASE / name / "main.py")
            module.SHOWCASE = base
            project = module.build_project(size, fps)
            # The public render path validates semantics, preflights inputs, and publishes output.
            output = args.output_dir / f"{name}.mp4"
            project.render(output, backend="cpu", overwrite=True, show_progress=False)
            probe(
                output,
                size,
                fps,
                duration,
                name == "audio-visualizer",
            )
            if args.update_results:
                shutil.copyfile(output, SHOWCASE / name / "render.mp4")
                if name != "readme-demo":
                    run(
                        "ffmpeg",
                        "-v",
                        "error",
                        "-i",
                        str(output),
                        "-filter_complex",
                        "[0:v]fps=8,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=64:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4",
                        "-loop",
                        "0",
                        "-y",
                        str(SHOWCASE / name / "preview.gif"),
                    )
                run(
                    "ffmpeg",
                    "-v",
                    "error",
                    "-ss",
                    "3",
                    "-i",
                    str(output),
                    "-frames:v",
                    "1",
                    "-vf",
                    "scale=640:-1",
                    "-map_metadata",
                    "-1",
                    "-q:v",
                    "3",
                    "-y",
                    str(SHOWCASE / name / "poster.jpg"),
                )
    if args.preview:
        run(
            "ffmpeg",
            "-v",
            "error",
            "-i",
            str(args.output_dir / "readme-demo.mp4"),
            "-filter_complex",
            "[0:v]fps=10,scale=640:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4",
            "-loop",
            "0",
            "-y",
            str(SHOWCASE / "readme-demo/preview.gif"),
        )


if __name__ == "__main__":
    main()
