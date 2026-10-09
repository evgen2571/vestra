"""Original periodic footage/audio exercising the complete stylization suite."""

import argparse
import subprocess
from pathlib import Path

from vestra import Project
from vestra.effects import (
    Ascii,
    Crt,
    Halftone,
    OrderedDither,
    PaletteMap,
    PixelSort,
    PseudoAscii,
)
from vestra.sources import Video
from vestra.effects.recipes import analog_monitor, halftone_print, sorted_neon

ROOT = Path(__file__).resolve().parents[3]
PERIOD = 4
PALETTE = ("#071827", "#27565d", "#69b49c", "#fff0c0")
LOOKS = (
    "ascii",
    "custom-ascii",
    "pseudo",
    "halftone",
    "horizontal",
    "vertical",
    "crt",
    "palette",
    "dither",
    "analog-monitor",
    "halftone-print",
    "sorted-neon",
)


def prepare_assets(directory: Path, size: tuple[int, int], fps: int) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    # Every temporal frequency completes an integer number of cycles in four seconds.
    tone = (
        "32+90*Y/H+40*sin(2*PI*X/W)+10*cos(2*PI*Y/7)"
        "+if(lt(pow((X/W-(0.5+0.2*sin(2*PI*T/4)))/0.13,2)"
        "+pow((Y/H-0.5)/0.35,2),1),80,0)"
    )
    subprocess.run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            f"nullsrc=size={size[0]}x{size[1]}:rate={fps},geq=lum='{tone}':cb=128:cr=128",
            "-t",
            str(PERIOD),
            "-an",
            "-c:v",
            "ffv1",
            str(directory / "source.mkv"),
        ],
        check=True,
    )
    subprocess.run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "aevalsrc=0.09*sin(2*PI*110*t)*(0.65+0.35*cos(4*PI*t))"
            "+0.06*sin(2*PI*220*t)*(0.6+0.4*cos(2*PI*t/4))"
            "+0.06*sin(2*PI*275*t)*(0.6+0.4*cos(2*PI*t/4-2*PI/3))"
            "+0.06*sin(2*PI*330*t)*(0.6+0.4*cos(2*PI*t/4-4*PI/3))"
            f":sample_rate=48000:duration={PERIOD}",
            "-c:a",
            "pcm_s16le",
            str(directory / "music.wav"),
        ],
        check=True,
    )


def build_project(
    look: str, directory: Path, size: tuple[int, int], fps: int
) -> Project:
    project = Project(size=size, fps=fps, duration=2 * PERIOD, base_directory=directory)
    for start in (0, PERIOD):
        layer = project.root.add(
            Video("source.mkv", sizing="cover"), start=start, duration=PERIOD
        )
        if look == "ascii":
            effect = Ascii(
                color_mode="palette", palette=PALETTE, period=PERIOD, source_mix=0.12
            )
        elif look == "custom-ascii":
            effect = Ascii(
                " .oO@",
                font=ROOT / "tests/assets/VestraTest-Regular.ttf",
                color_mode="rainbow",
                period=PERIOD,
                mode="fill",
            )
        elif look == "pseudo":
            effect = PseudoAscii(color_mode="source", cell_width=6, cell_height=8)
        elif look == "halftone":
            effect = Halftone(
                cell_size=6, softness=0.7, foreground=PALETTE[-1], background=PALETTE[0]
            )
        elif look in ("horizontal", "vertical"):
            effect = PixelSort(direction=look, segment_length=64)
        elif look == "crt":
            effect = Crt(period=PERIOD, seed=37)
        elif look == "palette":
            effect = PaletteMap(mode="rainbow", period=PERIOD)
        elif look == "dither":
            effect = OrderedDither(PALETTE, matrix="bayer8", scale=1, period=PERIOD)
        elif look == "analog-monitor":
            effect = analog_monitor(period=PERIOD)
        elif look == "halftone-print":
            effect = halftone_print()
        elif look == "sorted-neon":
            effect = sorted_neon()
        else:
            raise ValueError(f"unknown look: {look}")
        for item in effect if isinstance(effect, tuple) else (effect,):
            attached = layer.effects.add(item)
            if hasattr(attached, "amount"):
                # Existing interpolation fades each stylization into its source and back.
                attached.amount.keyframe(0, 0.75)
                attached.amount.keyframe(PERIOD / 2, 1)
                attached.amount.keyframe(PERIOD, 0.75)
        project.audio.track().add("music.wav", start=start, trim_end=PERIOD)
    return project


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--look", choices=LOOKS, default="ascii")
    parser.add_argument("--backend", choices=("cpu", "wgpu"), default="cpu")
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    size, fps = ((320, 180), 6) if args.smoke else ((1920, 1080), 30)
    directory = ROOT / "target/stylized-showcase" / ("smoke" if args.smoke else "1080p")
    prepare_assets(directory, size, fps)
    project = build_project(args.look, directory, size, fps)
    project.snapshot().save(directory / f"{args.look}.json")
    output = args.output or directory / f"{args.look}-{args.backend}.mp4"
    project.render(str(output), backend=args.backend, overwrite=True)


if __name__ == "__main__":
    main()
