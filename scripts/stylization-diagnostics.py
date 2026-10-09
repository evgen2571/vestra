#!/usr/bin/env python3
"""Build parity differences and temporal contact sheets from stylization tests.

Run with the visual-regression skill's isolated Pillow environment:
uv run --no-project --with 'Pillow>=10,<13' python scripts/stylization-diagnostics.py
"""

import argparse
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKILL_SCRIPTS = ROOT / ".agents/skills/visual-regression/scripts"


def load_script(name):
    spec = importlib.util.spec_from_file_location(name, SKILL_SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def frame_pairs(directory):
    pairs = []
    for cpu in directory.glob("cpu-*.png"):
        timestamp = int(cpu.stem.removeprefix("cpu-"))
        gpu = directory / f"wgpu-{timestamp}.png"
        if not gpu.is_file():
            raise ValueError(f"missing WGPU counterpart for {cpu}")
        pairs.append((timestamp, cpu, gpu))
    return sorted(pairs)


def build_report(directory, output, compare, create_sheet):
    pairs = frame_pairs(directory)
    if not pairs:
        raise ValueError(f"no rendered CPU/WGPU pairs in {directory}")
    output.mkdir(parents=True, exist_ok=True)
    report = {"fixture": directory.name, "parity": [], "temporal": []}
    metadata = directory / "adapter.json"
    report["adapter"] = json.loads(metadata.read_text()) if metadata.exists() else None
    resources = directory / "resources.json"
    report["resources"] = (
        json.loads(resources.read_text()) if resources.exists() else None
    )
    images, labels = [], []
    for timestamp, cpu, gpu in pairs:
        parity = compare(cpu, gpu, output / "parity" / str(timestamp), 1)
        report["parity"].append({"time_ns": timestamp, **parity})
        images.extend([cpu, gpu])
        labels.extend([f"CPU {timestamp / 1e9:.9f}s", f"WGPU {timestamp / 1e9:.9f}s"])
    for previous, current in zip(pairs, pairs[1:]):
        step = {"from_ns": previous[0], "to_ns": current[0]}
        for index, backend in [(1, "cpu"), (2, "wgpu")]:
            step[backend] = compare(
                previous[index],
                current[index],
                output / "temporal" / backend / str(current[0]),
                0,
            )
        report["temporal"].append(step)
    create_sheet(images, labels, output / "contact.png", 2, 400, 225)
    (output / "report.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--frames", type=Path, default=ROOT / "target/stylization/frames"
    )
    parser.add_argument(
        "--output", type=Path, default=ROOT / "target/stylization/diagnostics"
    )
    parser.add_argument(
        "--fixture", action="append", help="exact fixture directory; repeatable"
    )
    args = parser.parse_args()
    compare = load_script("compare_frames").compare
    create_sheet = load_script("create_contact_sheet").create_sheet
    try:
        directories = sorted({frame.parent for frame in args.frames.rglob("cpu-*.png")})
        if args.fixture:
            directories = [args.frames / name for name in args.fixture]
        else:
            directories = [path for path in directories if list(path.glob("cpu-*.png"))]
        if not directories:
            parser.error("no rendered fixtures; run the gpu_stylization tests first")
        for directory in directories:
            report = build_report(
                directory,
                args.output / directory.relative_to(args.frames),
                compare,
                create_sheet,
            )
            print(
                f"{directory.name}: {len(report['parity'])} paired frames; {args.output / directory.relative_to(args.frames) / 'contact.png'}"
            )
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
