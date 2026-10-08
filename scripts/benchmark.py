"""Run and compare Vestra's existing release benchmark."""

import argparse
import hashlib
import json
import math
import os
import subprocess
from datetime import datetime
from pathlib import Path
from statistics import median

ROOT = Path(__file__).resolve().parents[1]


def source_identity() -> str:
    """Fingerprint build inputs, including untracked benchmark implementation."""
    paths = (
        subprocess.check_output(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            cwd=ROOT,
        )
        .decode()
        .split("\0")
    )
    digest = hashlib.sha256()
    for name in sorted(set(paths)):
        if name and (
            name.startswith(("crates/", ".cargo/", "benchmarks/projects/"))
            or name
            in {
                "Cargo.toml",
                "Cargo.lock",
                "rust-toolchain.toml",
                "scripts/benchmark.py",
                "benchmarks/suites.json",
            }
        ):
            path = ROOT / name
            digest.update(name.encode() + b"\0")
            digest.update(
                hashlib.sha256(path.read_bytes()).digest()
                if path.is_file()
                else b"deleted"
            )
    return digest.hexdigest()


def compatibility(record: dict) -> dict:
    """Check every sample selected the same workload and execution backend."""
    if record["benchmark_schema_version"] != 1:
        raise ValueError("incompatible benchmark schema")
    if not record["samples"]:
        raise ValueError("benchmark has no samples")
    fields = (
        "width",
        "height",
        "frame_rate",
        "total_frames",
        "timing_scope",
        "requested_render_backend",
        "render_backend",
        "encoder_backend",
        "adapter",
    )
    configurations = [
        {key: sample["result"].get(key) for key in fields}
        for sample in record["samples"]
    ]
    if any(config != configurations[0] for config in configurations):
        raise ValueError("incompatible backend or workload within samples")
    return {
        "scenario": record["scenario"],
        "workload_id": record["workload_id"],
        "warmups": record["warmups"],
        "environment": {
            key: value
            for key, value in record["environment"].items()
            if key not in {"revision", "dirty", "diff_sha256", "recorded_at"}
        },
        "execution": configurations[0],
    }


def metrics(sample: dict) -> dict:
    """Flatten only measured timing and resource fields, preserving nulls."""
    values = {"wall_ms": sample["wall_ms"]}
    for category in ("timings", "performance"):
        for key, value in sample["result"][category].items():
            if category == "performance" and isinstance(value, (str, bool)):
                continue
            if value is not None and (
                isinstance(value, bool)
                or not isinstance(value, (int, float))
                or not math.isfinite(value)
                or value < 0
            ):
                raise ValueError(f"invalid measurement: {category}.{key}")
            values[f"{category}.{key}"] = value
    wall = values["wall_ms"]
    if (
        isinstance(wall, bool)
        or not isinstance(wall, (int, float))
        or not math.isfinite(wall)
        or wall < 0
    ):
        raise ValueError("invalid measurement: wall_ms")
    return values


def compare(before: dict, after: dict) -> list[dict]:
    """Return median deltas; reject measurements whose context differs."""
    if compatibility(before) != compatibility(after):
        raise ValueError("incompatible benchmark workload, environment, or backend")
    old = [metrics(sample) for sample in before["samples"]]
    new = [metrics(sample) for sample in after["samples"]]
    if any(values.keys() != old[0].keys() for values in old + new):
        raise ValueError("incompatible metric fields")
    rows = []
    for key in old[0]:
        values = [sample[key] for sample in old + new]
        if all(value is None for value in values):
            continue
        if any(value is None for value in values):
            raise ValueError(f"incompatible availability for {key}")
        previous = median(sample[key] for sample in old)
        current = median(sample[key] for sample in new)
        delta = current - previous
        rows.append(
            {
                "metric": key,
                "before": previous,
                "after": current,
                "delta": delta,
                "percent": delta / previous * 100 if previous else None,
            }
        )
    return rows


def main() -> None:
    """Compare saved records from the command line."""
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run", help="run a release benchmark suite")
    manifest = json.loads((ROOT / "benchmarks/suites.json").read_text())
    run.add_argument("--suite", choices=tuple(manifest["suites"]), default="canonical")
    run.add_argument("--backend", choices=("cpu", "hardware-wgpu"), default="cpu")
    run.add_argument(
        "--executable", type=Path, help="prebuilt release benchmark executable"
    )
    run.add_argument(
        "--output", type=Path, required=True, help="new directory for results and logs"
    )
    comparison = commands.add_parser(
        "compare", help="compare two benchmark JSON records"
    )
    comparison.add_argument("before", type=Path)
    comparison.add_argument("after", type=Path)
    comparison.add_argument(
        "--json", action="store_true", help="emit machine-readable deltas"
    )
    args = parser.parse_args()
    try:
        if args.command == "run":
            run_suite(args)
            return
        rows = compare_documents(
            json.loads(args.before.read_text()), json.loads(args.after.read_text())
        )
    except (
        OSError,
        ValueError,
        KeyError,
        TypeError,
        subprocess.CalledProcessError,
    ) as error:
        parser.exit(2, f"benchmark: {error}\n")
    if args.json:
        print(json.dumps(rows, indent=2, allow_nan=False))
    else:
        print("Metric | Before | After | Delta | Change")
        print("--- | ---: | ---: | ---: | ---:")
        for row in rows:
            percent = "n/a" if row["percent"] is None else f"{row['percent']:+.2f}%"
            print(
                f"{row['metric']} | {row['before']:g} | {row['after']:g} | {row['delta']:+g} | {percent}"
            )


def compare_documents(before: dict, after: dict) -> list[dict]:
    """Compare standalone records or complete suites without dropping workloads."""
    if "suite_schema_version" not in before and "suite_schema_version" not in after:
        return compare(before, after)
    if any(document.get("suite_schema_version") != 1 for document in (before, after)):
        raise ValueError("incompatible suite schema")
    if before["definition"] != after["definition"]:
        raise ValueError("incompatible suite definition")
    old = {record["scenario"]: record for record in before["records"]}
    new = {record["scenario"]: record for record in after["records"]}
    expected = before["definition"]["scenarios"]
    if (
        set(old) != set(expected)
        or set(new) != set(expected)
        or len(old) != len(before["records"])
        or len(new) != len(after["records"])
    ):
        raise ValueError("incomplete or duplicate suite workloads")
    rows = []
    for scenario in expected:
        for document, record in ((before, old[scenario]), (after, new[scenario])):
            validate_settings(record, document["definition"]["settings"])
        for row in compare(old[scenario], new[scenario]):
            rows.append({**row, "metric": f"{scenario}/{row['metric']}"})
    return rows


def validate_settings(record: dict, settings: dict) -> None:
    """Require each record to match the suite's declared measurement settings."""
    if (
        len(record["samples"]) != settings["samples"]
        or record["warmups"] != settings["warmups"]
    ):
        raise ValueError("incomplete or incompatible suite sample count")
    for sample in record["samples"]:
        if any(sample["result"][key] != settings[key] for key in ("width", "height")):
            raise ValueError("incompatible suite resolution")


def build_benchmark(backend: str) -> Path:
    """Ask Cargo for its exact executable instead of guessing a hashed filename."""
    features = "cpu" if backend == "cpu" else "cpu,wgpu"
    result = subprocess.run(
        [
            "cargo",
            "bench",
            "-p",
            "vestra",
            "--bench",
            "animation_effects",
            "--no-default-features",
            "--features",
            features,
            "--no-run",
            "--locked",
            "--message-format=json",
        ],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if (
            message.get("reason") == "compiler-artifact"
            and message.get("target", {}).get("name") == "animation_effects"
            and message.get("executable")
        ):
            return Path(message["executable"])
    raise ValueError("Cargo did not report the benchmark executable")


def suite_configuration(manifest: dict, name: str) -> tuple[dict, list[str]]:
    """Resolve suite-local workloads without changing canonical suite settings."""
    settings = dict(manifest["suites"][name])
    scenarios = list(settings.pop("scenarios", manifest["scenarios"]))
    return settings, scenarios


def run_suite(args: argparse.Namespace) -> None:
    """Run workloads serially and publish a suite only after every render succeeds."""
    manifest = json.loads((ROOT / "benchmarks/suites.json").read_text())
    settings, scenarios = suite_configuration(manifest, args.suite)
    source = source_identity()
    executable = (
        args.executable.resolve() if args.executable else build_benchmark(args.backend)
    )
    args.output.mkdir(parents=True, exist_ok=False)
    provenance = {
        "source_sha256": source,
        "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
        "built_by_runner": args.executable is None,
        "recorded_at": datetime.now().astimezone().isoformat(),
    }
    records = []
    for scenario in scenarios:
        print(f"Measuring {scenario} ({args.suite}, {args.backend})", flush=True)
        report = (args.output / f"{scenario}.json").resolve()
        environment = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith("VESTRA_BENCH_")
        }
        environment.update(
            {
                f"VESTRA_BENCH_{key.upper()}": str(value)
                for key, value in settings.items()
            }
        )
        environment.update(
            VESTRA_BENCH_SCENARIO=scenario,
            VESTRA_BENCH_REPORT=str(report),
            VESTRA_BENCH_BACKEND="cpu" if args.backend == "cpu" else "wgpu",
        )
        with (args.output / f"{scenario}.log").open("w") as log:
            subprocess.run(
                [str(executable)],
                cwd=ROOT,
                env=environment,
                stdout=log,
                stderr=subprocess.STDOUT,
                check=True,
            )
        record = json.loads(report.read_text())
        if record["scenario"] != scenario:
            raise ValueError("incompatible scenario returned by benchmark")
        validate_settings(record, settings)
        compatibility(record)
        for sample in record["samples"]:
            require_backend(sample["result"], args.backend)
        records.append(record)
    suite = {
        "suite_schema_version": 1,
        "provenance": provenance,
        "definition": {
            "version": manifest["suite_version"],
            "name": args.suite,
            "backend": args.backend,
            "settings": settings,
            "scenarios": scenarios,
        },
        "records": records,
    }
    if source_identity() != source:
        raise ValueError("source changed during measurement; suite not published")
    (args.output / "suite.json").write_text(
        json.dumps(suite, indent=2, allow_nan=False) + "\n"
    )
    print(f"Saved {args.output / 'suite.json'}")


def require_backend(result: dict, backend: str) -> None:
    """Keep software adapters out of hardware measurement suites."""
    if backend == "hardware-wgpu":
        adapter = result.get("adapter") or {}
        if result["render_backend"] != "wgpu" or adapter.get("device_type") not in {
            "integratedgpu",
            "discretegpu",
        }:
            raise ValueError(
                "hardware-GPU blocked: render did not select a confirmed hardware adapter"
            )
    elif result["render_backend"] != "cpu":
        raise ValueError("CPU suite selected a different backend")


if __name__ == "__main__":
    main()
