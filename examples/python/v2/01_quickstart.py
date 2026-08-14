"""Build a small editor project and render it without invoking the CLI."""

from __future__ import annotations

import argparse
import tempfile
from pathlib import Path

from vestra import Project
from vestra.effects import Vignette
from vestra.sources import Image

ROOT = Path(__file__).resolve().parents[3]


def build_project() -> Project:
    project = Project(size=(160, 90), fps=12, duration=1, base_directory=ROOT)
    layer = project.root.add(
        Image("examples/assets/red.png", sizing="cover"),
        duration=1,
        id="background",
    )
    layer.effects.add(Vignette(0.2, 1.0, 0.5, "#000000"))
    return project


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, help="write the MP4 here")
    args = parser.parse_args()
    project = build_project()
    report = project.validate()
    print(f"valid={report.is_valid}")
    if not report.is_valid:
        raise SystemExit("project validation failed")
    if args.output is None:
        with tempfile.TemporaryDirectory(prefix="vestra-v2-") as directory:
            result = project.render(Path(directory) / "quickstart.mp4", backend="cpu")
            print(f"backend={result.selected_backend} output={result.output_path}")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        result = project.render(args.output, backend="cpu", overwrite=True)
        print(f"backend={result.selected_backend} output={result.output_path}")


if __name__ == "__main__":
    main()
