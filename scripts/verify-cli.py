#!/usr/bin/env python3
"""Check a CLI release checksum, then smoke-test the extracted executable."""

import argparse
from contextlib import nullcontext
import hashlib
import json
from pathlib import Path
import shutil
import stat
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile


def verify_checksum(archive: Path) -> None:
    checksum = archive.with_name(archive.name + ".sha256").read_text().split()
    with archive.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if checksum != [digest, archive.name]:
        raise ValueError(f"CLI archive checksum mismatch: {archive.name}")


def extract(archive: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)

    def path_for(name: str) -> Path:
        path = (destination / name).resolve()
        if (
            "\\" in name
            or ":" in name
            or not path.is_relative_to(destination.resolve())
        ):
            raise ValueError(f"unsafe archive path: {name}")
        return path

    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as bundle:
            for member in bundle.infolist():
                path = path_for(member.filename)
                mode = stat.S_IFMT(member.external_attr >> 16)
                if mode not in (0, stat.S_IFREG, stat.S_IFDIR):
                    raise ValueError(f"unsafe archive member: {member.filename}")
                if member.is_dir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with bundle.open(member) as source, path.open("wb") as output:
                        shutil.copyfileobj(source, output)
    else:
        with tarfile.open(archive, "r:gz") as bundle:
            for member in bundle:
                path = path_for(member.name)
                if member.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                elif member.isfile():
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with (
                        bundle.extractfile(member) as source,
                        path.open("wb") as output,
                    ):
                        shutil.copyfileobj(source, output)
                    path.chmod(member.mode & 0o777)
                else:
                    raise ValueError(f"unsafe archive member: {member.name}")


def smoke_project() -> dict:
    return {
        "schema_version": 1,
        "name": "Packaged CLI smoke",
        "output": {
            "path": "cli-smoke.mp4",
            "width": 32,
            "height": 32,
            "frame_rate": "4/1",
            "background": "#000000",
            "quality": "balanced",
            "audio": False,
            "duration_mode": "automatic",
        },
        "assets": [],
        "visual": {
            "clips": [
                {
                    "id": "red",
                    "source": {"type": "solid_color", "colour": "#FF0000"},
                    "start": 0,
                    "duration": 0.5,
                    "layer": 0,
                    "opacity": {"base_value": 1.0},
                },
                {
                    "id": "blue",
                    "source": {"type": "solid_color", "colour": "#0000FF"},
                    "start": 0.5,
                    "duration": 0.5,
                    "layer": 0,
                    "opacity": {"base_value": 1.0},
                },
            ]
        },
    }


def verify_probe(probe: dict) -> None:
    streams = probe.get("streams", [])
    expected = {
        "codec_name": "h264",
        "pix_fmt": "yuv420p",
        "width": 32,
        "height": 32,
        "avg_frame_rate": "4/1",
        "nb_read_frames": "4",
    }
    if len(streams) != 1 or any(
        streams[0].get(key) != value for key, value in expected.items()
    ):
        raise ValueError(f"unexpected rendered video stream: {streams}")
    if abs(float(streams[0]["duration"]) - 1.0) > 0.001:
        raise ValueError(f"unexpected rendered video duration: {streams[0]}")
    timestamps = [
        float(frame["best_effort_timestamp_time"]) for frame in probe.get("frames", [])
    ]
    if len(timestamps) != 4 or any(
        abs(actual - index / 4) > 0.001 for index, actual in enumerate(timestamps)
    ):
        raise ValueError(
            f"unexpected rendered video presentation timestamps: {timestamps}"
        )


def smoke(binary: Path, output_dir: Path, expected_version: str) -> None:
    def run(*args: str) -> str:
        try:
            result = subprocess.run(
                [str(binary), *args],
                cwd=output_dir,
                check=True,
                stdout=subprocess.PIPE,
                text=True,
            )
        except subprocess.CalledProcessError as error:
            raise RuntimeError(
                f"packaged CLI {args[0]} failed:\n{error.stdout}"
            ) from error
        return result.stdout

    version = run("--version").strip()
    if version != f"ve {expected_version}":
        raise ValueError(
            f"unexpected packaged CLI version: {version!r}, expected {expected_version}"
        )
    project = output_dir / "cli-smoke.json"
    project.write_text(json.dumps(smoke_project()), encoding="utf-8")
    validation = json.loads(run("validate", str(project), "--format", "json"))
    if validation.get("status") != "success":
        raise ValueError(f"packaged CLI validation failed: {validation}")
    output = output_dir / "cli-smoke.mp4"
    render = json.loads(
        run(
            "render",
            str(project),
            "--output",
            str(output),
            "--render-backend",
            "cpu",
            "--progress",
            "none",
            "--format",
            "json",
        )
    )
    if (
        render.get("status"),
        render.get("render_backend"),
        render.get("total_frames"),
    ) != ("success", "cpu", 4):
        raise ValueError(f"packaged CLI CPU render failed: {render}")
    probe = subprocess.run(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_frames",
            "-show_frames",
            "-show_entries",
            "stream=codec_name,pix_fmt,width,height,avg_frame_rate,nb_read_frames,duration:frame=best_effort_timestamp_time",
            "-of",
            "json",
            str(output),
        ],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    verify_probe(json.loads(probe.stdout))
    print(f"CLI smoke passed: {version}; CPU; H264/yuv420p; 32x32; 4 frames; 1 second")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--expected-version")
    parser.add_argument(
        "--output-dir", type=Path, help="keep the project and rendered MP4 here"
    )
    args = parser.parse_args()
    archive = args.archive.resolve()
    expected_version = args.expected_version
    if expected_version is None:
        manifest = Path(__file__).resolve().parents[1] / "Cargo.toml"
        expected_version = tomllib.loads(manifest.read_text())["workspace"]["package"][
            "version"
        ]
    verify_checksum(archive)
    with tempfile.TemporaryDirectory(prefix="vestra-cli-extracted-") as directory:
        extracted = Path(directory)
        extract(archive, extracted)
        binaries = [
            path
            for path in extracted.rglob("*")
            if path.name in ("ve", "ve.exe") and path.is_file()
        ]
        if len(binaries) != 1:
            raise ValueError(
                f"expected one packaged CLI executable, found {len(binaries)}"
            )
        root = binaries[0].parent
        for name in (
            "LICENSE",
            "THIRD_PARTY_NOTICES.md",
            "README.txt",
            "licenses/FFmpeg-LGPL-2.1.txt",
            "licenses/DejaVuSans.txt",
        ):
            if not (root / name).is_file():
                raise ValueError(f"CLI archive is missing {name}")
        context = (
            nullcontext(args.output_dir)
            if args.output_dir
            else tempfile.TemporaryDirectory(prefix="vestra-cli-smoke-")
        )
        with context as output_directory:
            output_dir = Path(output_directory).resolve()
            output_dir.mkdir(parents=True, exist_ok=True)
            smoke(binaries[0], output_dir, expected_version)


if __name__ == "__main__":
    main()
