"""Fine-detail dither and ASCII checks on an original moving synthetic portrait.

Run: VESTRA_WGPU_BACKEND=gl uv run python examples/showcase/stylized-effects/reference.py
Artifacts stay in target/stylized-reference; no downloaded source image is used.
"""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

from main import PALETTES
from vestra import Project
from vestra.effects import Ascii, OrderedDither
from vestra.sources import Video

ROOT = Path(__file__).resolve().parents[3]


def prepare_source(directory: Path, size: tuple[int, int]) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    # Stationary shadows and whole-pixel periodic subject motion separate scene
    # changes from random threshold motion. Fine hair/cloth bands are intentional.
    coordinates = "st(0,X/W-0.53-floor(8*sin(2*PI*T/4))/W);st(1,Y/H);"
    face = "lt(pow(ld(0)/0.125,2)+pow((ld(1)-0.38)/0.25,2),1)"
    coat = "lt(pow(ld(0)/0.29,2)+pow((ld(1)-0.94)/0.46,2),1)"
    eyes = "lt(abs(abs(ld(0))-0.05),0.023)*lt(abs(ld(1)-0.35),0.009)"
    mouth = "lt(abs(ld(0)),0.046)*lt(abs(ld(1)-0.49),0.006)"
    hair = "gt(ld(1),0.14)*lt(ld(1),0.7)*lt(abs(ld(0)),0.16)*gt(abs(ld(0)),0.105)"
    tone = (
        coordinates + f"if({face},175+45*cos(ld(0)*15)-60*ld(0)/0.125"
        f"-95*({eyes})-65*({mouth})+5*cos(2*PI*Y/9),"
        f"if({hair},50+22*cos(2*PI*X/5)+25*ld(1),"
        f"if({coat},55+45*cos(ld(0)*8)+14*cos(2*PI*Y/11)"
        "+6*cos(2*PI*X/7),7)))"
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
            f"nullsrc=size={size[0]}x{size[1]}:rate=30,geq=lum='{tone}':cb=128:cr=128",
            "-t",
            "4",
            "-an",
            "-c:v",
            "ffv1",
            str(directory / "source.mkv"),
        ],
        check=True,
    )


def build_project(directory: Path, size: tuple[int, int], effect=None) -> Project:
    project = Project(size=size, fps=30, duration=4, base_directory=directory)
    layer = project.root.add(Video("source.mkv", sizing="cover"), duration=4)
    if effect is not None:
        layer.effects.add(effect)
    return project


def save_frame(path: Path, size: tuple[int, int], data: bytes) -> None:
    subprocess.run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            f"{size[0]}x{size[1]}",
            "-i",
            "pipe:0",
            "-frames:v",
            "1",
            str(path),
        ],
        input=data,
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--backend", choices=("cpu", "wgpu"), default="cpu")
    parser.add_argument("--full-hd", action="store_true")
    args = parser.parse_args()
    size = (1920, 1080) if args.full_hd else (640, 360)
    directory = ROOT / "target/stylized-reference" / f"{size[0]}-{args.backend}"
    prepare_source(directory, size)
    projects = {"source": build_project(directory, size)}
    for name, colors in PALETTES.items():
        projects[name] = build_project(
            directory,
            size,
            OrderedDither(
                colors,
                matrix="bayer8",
                scale=1,
                input_scale=1,
                stops=(0, 0.22, 0.6, 1),
                input_gamma=0.9,
                input_detail=1,
                input_detail_radius=1,
            ),
        )
    for mode in ("monochrome", "source", "palette"):
        projects[f"ascii-{mode}"] = build_project(
            directory,
            size,
            Ascii(
                color_mode=mode,
                palette=PALETTES["ocean"],
                source_mix=0,
                cell_width=6,
                cell_height=8,
                mode="fill",
            ),
        )
    report = {}
    palette_geometry = []
    for name, project in projects.items():
        project.snapshot().save(directory / f"{name}.json")
        prepared = project.prepare(backend=args.backend)
        samples = {}
        for time in (0, 1 / 30, 2 / 30, 0.1, 0.25, 1, 2, 3.9, 0.25):
            data = prepared.render_frame_seconds(time).to_bytes()
            if time in samples:
                assert samples[time] == data, f"{name}: random-access frame changed"
            samples[time] = data
            save_frame(directory / f"{name}-{time:.3f}.png", size, data)
        assert samples[0] == samples[1 / 30] == samples[2 / 30], (
            f"{name}: stationary flicker"
        )
        assert samples[0] != samples[0.1], f"{name}: missing subject motion"
        if name in PALETTES:
            indices = {
                bytes.fromhex(c[1:]) + b"\xff": i for i, c in enumerate(PALETTES[name])
            }
            geometry = bytes(
                indices[samples[0][i : i + 4]] for i in range(0, len(samples[0]), 4)
            )
            palette_geometry.append(hashlib.sha256(geometry).hexdigest())
        values = samples[0][0::4]
        assert max(values) > 100 and min(values) <= 32, f"{name}: lost tonal range"
        assert len(set(values)) > 2, f"{name}: uniform/black output"
        report[name] = {
            "maximum_red": max(values),
            "distinct_red_values": len(set(values)),
            "visible_fraction": sum(v > 32 for v in values) / len(values),
            "adjacent_mae": [
                sum(abs(a - b) for a, b in zip(samples[t], samples[0])) / len(data)
                for t in (1 / 30, 2 / 30, 0.1)
            ],
            "random_access": "identical",
            "backend": str(prepared.preparation_report.selected_backend),
            "adapter": (
                {
                    "name": prepared.preparation_report.adapter.adapter_name,
                    "graphics_backend": str(
                        prepared.preparation_report.adapter.graphics_backend
                    ),
                }
                if prepared.preparation_report.adapter is not None
                else None
            ),
        }
        del prepared
        if name in ("source", "mono", "ember", "ocean", "ascii-source"):
            project.render(
                str(directory / f"{name}.mp4"), backend=args.backend, overwrite=True
            )
    assert len(set(palette_geometry)) == 1, "palette hue changed dither geometry"
    report["palette_geometry_sha256"] = palette_geometry[0]
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
