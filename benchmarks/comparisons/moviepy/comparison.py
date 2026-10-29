"""External comparison adapter for scripts/benchmark.py; generated evidence is local."""

import argparse
import hashlib
import importlib.metadata
import json
import math
import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from statistics import median, pstdev

from scripts.benchmark import ROOT, measure_process, require_backend, source_identity
from benchmarks.comparisons.moviepy.workloads import (
    adapter_class as adapter_class,
    render_moviepy,
    render_vestra,
    workload,
)

SOURCES = {
    "forest": "forward-fly-above-snow-forest.mp4",
    "valley": "forward-fly-through-road-to-snow-valley.mp4",
    "tear": "dropping-tear.mp4",
}
SCENARIOS = ("basic", "editing", "layered")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, value):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(
        json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8"
    )
    temporary.replace(path)


def probe(path):
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
        capture_output=True,
        text=True,
        check=True,
    )
    if result.stderr.strip():
        raise ValueError(f"invalid media {path}: {result.stderr.strip()}")
    return json.loads(result.stdout)


def discover(assets):
    sources = {}
    for key, name in SOURCES.items():
        p = assets / name
        if not p.is_file():
            raise ValueError(
                f"missing asset {name}; supply --assets DIRECTORY (default ~/drive/Assets)"
            )
        data = probe(p)
        video = next((s for s in data["streams"] if s["codec_type"] == "video"), None)
        if (
            not video
            or float(video.get("duration", data["format"].get("duration", 0))) < 12
        ):
            raise ValueError(
                f"{name}: requires a decodable video at least 12 seconds long"
            )
        sources[key] = dict(filename=name, sha256=digest(p), probe=data)
    return sources


def validate_output(path, definition):
    from fractions import Fraction

    data = probe(path)
    videos = [s for s in data["streams"] if s["codec_type"] == "video"]
    if len(videos) != 1 or any(s["codec_type"] == "audio" for s in data["streams"]):
        raise ValueError(f"{path.name}: expected one silent video stream")
    v = videos[0]
    expected = definition["duration"] * definition["fps"]
    if (v["width"], v["height"]) != (definition["width"], definition["height"]):
        raise ValueError(f"{path.name}: incorrect dimensions")
    if (
        Fraction(v["avg_frame_rate"]) != definition["fps"]
        or int(v.get("nb_read_frames", 0)) != expected
    ):
        raise ValueError(
            f"{path.name}: incorrect FPS/frame count; expected {expected} frames"
        )
    if abs(float(v["duration"]) - definition["duration"]) > 0.001:
        raise ValueError(f"{path.name}: incorrect duration")
    if v["codec_name"] != "h264" or v["pix_fmt"] != "yuv420p":
        raise ValueError(f"{path.name}: expected H264/yuv420p")
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-xerror",
            "-i",
            str(path),
            "-map",
            "0:v:0",
            "-f",
            "null",
            "-",
        ],
        check=True,
        capture_output=True,
    )
    return dict(
        status="ok",
        width=v["width"],
        height=v["height"],
        fps=v["avg_frame_rate"],
        duration=float(v["duration"]),
        frames=int(v["nb_read_frames"]),
        size_bytes=path.stat().st_size,
        sha256=digest(path),
        full_decode=True,
    )


def prepare(args):
    sources = discover(args.assets.expanduser().resolve())
    args.inputs.mkdir(parents=True, exist_ok=False)
    manifest = dict(
        schema_version=1,
        ffmpeg=version(["ffmpeg", "-version"]),
        sources=sources,
        settings=dict(
            width=1280,
            height=720,
            fps=30,
            duration=12,
            codec="libx264",
            crf=18,
            preset="veryfast",
            pixel_format="yuv420p",
            audio=False,
        ),
        prepared={},
    )
    for key, data in sources.items():
        output = args.inputs / f"{key}.mp4"
        command = [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-i",
            str(args.assets.expanduser() / data["filename"]),
            "-map",
            "0:v:0",
            "-an",
            "-vf",
            "fps=30,scale=1280:720:force_original_aspect_ratio=increase:flags=bicubic,crop=1280:720,setsar=1",
            "-frames:v",
            "360",
            "-c:v",
            "libx264",
            "-crf",
            "18",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "2",
            "-map_metadata",
            "-1",
            "-fflags",
            "+bitexact",
            "-flags:v",
            "+bitexact",
            str(output),
        ]
        subprocess.run(command, check=True)
        validation = validate_output(
            output, dict(width=1280, height=720, fps=30, duration=12)
        )
        manifest["prepared"][key] = dict(
            filename=output.name,
            sha256=digest(output),
            validation=validation,
            command=[
                "<assets>/" + data["filename"]
                if a == str(args.assets.expanduser() / data["filename"])
                else "<inputs>/" + output.name
                if a == str(output)
                else a
                for a in command
            ],
        )
    write_json(args.inputs / "manifest.json", manifest)
    print(f"Prepared {args.inputs}; originals unchanged")


def load_inputs(directory):
    manifest_path = directory / "manifest.json"
    if not manifest_path.is_file():
        raise ValueError(f"missing {manifest_path}; run moviepy prepare first")
    manifest = json.loads(manifest_path.read_text())
    if manifest.get("schema_version") != 1 or set(manifest["prepared"]) != set(SOURCES):
        raise ValueError(
            "incomplete or incompatible input manifest; prepare a fresh directory"
        )
    for item in manifest["prepared"].values():
        path = directory / item["filename"]
        if not path.is_file() or digest(path) != item["sha256"]:
            raise ValueError(
                f"prepared input changed or missing: {path}; prepare a fresh directory"
            )
    return manifest


def version(command):
    try:
        return subprocess.check_output(
            command, text=True, stderr=subprocess.STDOUT
        ).splitlines()[0]
    except (OSError, subprocess.CalledProcessError):
        return None


def environment():
    packages = {}
    for name in (
        "vestra",
        "moviepy",
        "numpy",
        "Pillow",
        "psutil",
        "imageio",
        "imageio-ffmpeg",
    ):
        try:
            packages[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            packages[name] = None
    cpu = platform.processor()
    if platform.system() == "Darwin":
        cpu = version(["sysctl", "-n", "machdep.cpu.brand_string"]) or cpu
    elif platform.system() == "Windows":
        cpu = (
            version(
                [
                    "powershell",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "(Get-CimInstance Win32_Processor).Name",
                ]
            )
            or cpu
        )
    if Path("/proc/cpuinfo").is_file():
        cpu = next(
            (
                line.split(":", 1)[1].strip()
                for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")
            ),
            cpu,
        )
    try:
        import psutil

        memory_bytes = psutil.virtual_memory().total
    except ImportError:
        memory_bytes = None
    return dict(
        memory_bytes=memory_bytes,
        recorded_at=datetime.now(timezone.utc).isoformat(),
        python=sys.version,
        platform=platform.platform(),
        machine=platform.machine(),
        cpu=cpu,
        logical_cpus=os.cpu_count(),
        affinity=sorted(os.sched_getaffinity(0))
        if hasattr(os, "sched_getaffinity")
        else None,
        packages=packages,
        ffmpeg=version(["ffmpeg", "-version"]),
        ffprobe=version(["ffprobe", "-version"]),
        rustc=version(["rustc", "--version"]),
        revision=version(["git", "rev-parse", "HEAD"]),
        dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT)),
        source_sha256=source_identity(),
        graphics_api=os.environ.get("VESTRA_WGPU_BACKEND"),
        native_extension_sha256=native_fingerprint(),
        ffmpeg_executable_sha256=digest(Path(shutil.which("ffmpeg")))
        if shutil.which("ffmpeg")
        else None,
    )


def native_fingerprint():
    try:
        from vestra import _native

        return digest(Path(_native.__file__))
    except ImportError:
        return None


def summarize(samples, expected):
    if len(samples) != expected or any(s["status"] != "ok" for s in samples):
        raise ValueError("incomplete or failed samples cannot form a successful result")
    result = {}
    for key in ("wall_ms", "effective_fps", "peak_tree_rss_bytes", "cpu_percent"):
        values = [s[key] for s in samples]
        if any(
            v is not None
            and (
                isinstance(v, bool)
                or not isinstance(v, (int, float))
                or not math.isfinite(v)
                or v < 0
            )
            for v in values
        ):
            raise ValueError(f"invalid measurement: {key}")
        result[key] = (
            None
            if any(v is None for v in values)
            else dict(
                median=median(values),
                min=min(values),
                max=max(values),
                stdev=pstdev(values),
            )
        )
    return result


def run(args):
    if args.samples < 1 or args.warmups < 0 or args.timeout <= 0:
        raise ValueError("samples/timeout must be positive and warmups nonnegative")
    if args.width < 2 or args.height < 2 or args.width % 2 or args.height % 2:
        raise ValueError("width and height must be positive even integers")
    manifest = load_inputs(args.inputs)
    args.output.mkdir(parents=True, exist_ok=False)
    identity = source_identity()
    record = dict(
        comparison_schema_version=1,
        status="running",
        environment=environment(),
        input_manifest=manifest,
        settings=dict(
            samples=args.samples,
            warmups=args.warmups,
            width=args.width,
            height=args.height,
            timeout=args.timeout,
        ),
        records=[],
    )
    report = args.output / "comparison.json"
    write_json(report, record)
    try:
        for scenario in args.scenarios:
            definition = workload(scenario, args.width, args.height)
            for engine in args.engines:
                row = dict(
                    scenario=scenario,
                    engine=engine,
                    definition=definition,
                    warmups=[],
                    samples=[],
                    summary=None,
                )
                record["records"].append(row)
                for index in range(args.warmups + args.samples):
                    warmup = index < args.warmups
                    stem = f"{scenario}-{engine}-" + (
                        f"warmup-{index}"
                        if warmup
                        else f"sample-{index - args.warmups}"
                    )
                    output = (args.output / f"{stem}.mp4").resolve()
                    print(f"Rendering {stem}", flush=True)
                    command = [
                        sys.executable,
                        str(ROOT / "scripts/benchmark.py"),
                        "moviepy",
                        "worker",
                        "--inputs",
                        str(args.inputs.resolve()),
                        "--scenario",
                        scenario,
                        "--engine",
                        engine,
                        "--output",
                        str(output),
                        "--width",
                        str(args.width),
                        "--height",
                        str(args.height),
                    ]
                    load_before = (
                        list(os.getloadavg()) if hasattr(os, "getloadavg") else None
                    )
                    sample = measure_process(
                        command, args.output / f"{stem}.log", timeout=args.timeout
                    )
                    sample.update(
                        status="failed",
                        output=output.name,
                        host_loadavg_before=load_before,
                        host_loadavg_after=list(os.getloadavg())
                        if hasattr(os, "getloadavg")
                        else None,
                    )
                    row["warmups" if warmup else "samples"].append(sample)
                    write_json(report, record)
                    if sample["returncode"] or sample["timed_out"]:
                        raise ValueError(
                            f"{stem} failed (timeout={sample['timed_out']}); see {stem}.log"
                        )
                    sample["result"] = json.loads(
                        output.with_suffix(".result.json").read_text()
                    )
                    if engine != "moviepy-cpu":
                        require_backend(
                            sample["result"],
                            "cpu" if engine == "vestra-cpu" else "hardware-wgpu",
                        )
                        if sample["result"]["fallback"]:
                            raise ValueError(
                                "backend fallback is ineligible for comparison"
                            )
                    sample["validation"] = validate_output(output, definition)
                    sample["effective_fps"] = (
                        definition["duration"] * 30 / (sample["wall_ms"] / 1000)
                    )
                    sample["status"] = "ok"
                    write_json(report, record)
                row["summary"] = summarize(row["samples"], args.samples)
        if source_identity() != identity:
            raise ValueError("source changed during measurements; results invalid")
        load_inputs(args.inputs)
        final_environment = environment()
        stable_fields = (
            "packages",
            "native_extension_sha256",
            "ffmpeg_executable_sha256",
            "graphics_api",
        )
        if any(final_environment[k] != record["environment"][k] for k in stable_fields):
            raise ValueError("runtime changed during measurements; results invalid")
        record["status"] = "ok"
    except Exception as error:
        record.update(status="failed", error=str(error))
        raise
    finally:
        write_json(report, record)
    print(
        f"Saved {report}; wall time includes worker startup/import/build/render/finalization, validation excluded"
    )


def report(runs, verification=None):
    """Print a local table; refuse failed captures rather than showing partial wins."""
    documents = [json.loads((p / "comparison.json").read_text()) for p in runs]
    if any(d.get("status") != "ok" for d in documents):
        raise ValueError("failed or incomplete comparison run")
    from benchmarks.comparisons.moviepy.validation import eligible

    eligible(runs, documents[0]["input_manifest"])
    if verification is not None:
        evidence = json.loads(verification.read_text())
        expected = sorted(digest(p / "comparison.json") for p in runs)
        if (
            evidence.get("status") != "ok"
            or sorted(evidence.get("captures", [])) != expected
        ):
            raise ValueError(
                "missing, failed or incompatible visual verification evidence"
            )
        print("Decoded-frame verification passed for these captures.\n")
    else:
        print(
            "Timing capture only: visual equivalence is UNVERIFIED. Run moviepy verify before sharing.\n"
        )
    print(
        "Scenario | Engine | E2E median seconds (min–max) | FPS | Tree RSS MiB | CPU %"
    )
    print("--- | --- | ---: | ---: | ---: | ---:")
    for d in documents:
        for row in d["records"]:
            summary = summarize(row["samples"], d["settings"]["samples"])
            wall = summary["wall_ms"]

            def value(key, divisor=1):
                v = summary[key]
                return "unavailable" if v is None else f"{v['median'] / divisor:.2f}"

            print(
                f"{row['scenario']} | {row['engine']} | "
                f"{wall['median'] / 1000:.2f} ({wall['min'] / 1000:.2f}–{wall['max'] / 1000:.2f}) | "
                f"{value('effective_fps')} | {value('peak_tree_rss_bytes', 2**20)} | {value('cpu_percent')}"
            )
    print(
        "\nE2E includes worker startup/imports, project construction, decode, render and H264 publication. "
        "Input preparation and output verification are excluded. CPU %=one core at 100%; RSS is a 20ms sampled lower bound."
    )
    print(
        "Raw comparison.json retains versions, source/input/binary fingerprints, actual backend/adapter, "
        "per-sample measurements, output validation and stage timings. MoviePy lacks comparable internal stage counters."
    )
    for d in documents:
        e = d["environment"]
        print(
            f"\nEnvironment: {e['cpu']}; {e['platform']}; Python {e['python'].split()[0]}; {e['packages']}; {e['ffmpeg']}"
        )
        for row in d["records"]:
            result = row["samples"][0]["result"]
            print(
                f"{row['scenario']}/{row['engine']}: actual={result['render_backend']}, adapter={result['adapter']}"
            )


def add_arguments(parser):
    commands = parser.add_subparsers(dest="moviepy_command", required=True)
    for name in ("report", "verify"):
        p = commands.add_parser(
            name,
            help="summarize results"
            if name == "report"
            else "compare decoded boundary frames",
        )
        p.add_argument("runs", type=Path, nargs="+")
        if name == "report":
            p.add_argument(
                "--verification",
                type=Path,
                help="successful metrics.json from verify for exactly these captures",
            )
        if name == "verify":
            p.add_argument("--output", type=Path, required=True)
            p.add_argument("--inputs", type=Path, default=Path("target/moviepy-inputs"))
    prep = commands.add_parser(
        "prepare", help="prepare shared silent 720p30 input clips"
    )
    prep.add_argument("--assets", type=Path, default=Path("~/drive/Assets"))
    prep.add_argument("--inputs", type=Path, default=Path("target/moviepy-inputs"))
    run_parser = commands.add_parser(
        "run", help="serial end-to-end measurements; use a fresh output directory"
    )
    run_parser.add_argument(
        "--engines",
        nargs="+",
        choices=("vestra-cpu", "moviepy-cpu", "hardware-wgpu"),
        default=["vestra-cpu", "moviepy-cpu"],
    )
    run_parser.add_argument(
        "--scenarios", nargs="+", choices=SCENARIOS, default=list(SCENARIOS)
    )
    run_parser.add_argument("--warmups", type=int, default=1)
    run_parser.add_argument("--samples", type=int, default=5)
    run_parser.add_argument("--timeout", type=float, default=900)
    for p in (run_parser, commands.add_parser("worker", help=argparse.SUPPRESS)):
        p.add_argument("--inputs", type=Path, default=Path("target/moviepy-inputs"))
        p.add_argument("--output", type=Path, required=True)
        p.add_argument("--width", type=int, default=1920)
        p.add_argument("--height", type=int, default=1080)
        if p is not run_parser:
            p.add_argument(
                "--engine",
                choices=("vestra-cpu", "moviepy-cpu", "hardware-wgpu"),
                required=True,
            )
            p.add_argument("--scenario", choices=SCENARIOS, required=True)


def main(args):
    if args.moviepy_command == "report":
        report(args.runs, args.verification)
    elif args.moviepy_command == "verify":
        from benchmarks.comparisons.moviepy.validation import verify

        verify(args.runs, args.output, args.inputs)
    elif args.moviepy_command == "prepare":
        prepare(args)
    elif args.moviepy_command == "run":
        run(args)
    else:
        definition = workload(args.scenario, args.width, args.height)
        if args.engine == "moviepy-cpu":
            result = render_moviepy(definition, args.inputs, args.output)
        else:
            result = render_vestra(
                definition,
                args.inputs,
                args.output,
                "hardware-wgpu" if args.engine == "hardware-wgpu" else "cpu",
            )
        write_json(args.output.with_suffix(".result.json"), result)
