"""Thin orchestration around canonical benchmark and MoviePy adapters."""

import argparse
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import traceback

from benchmarks.collector import host, reporting
from benchmarks.comparisons.moviepy.comparison import digest, write_json
from scripts.benchmark import ROOT, source_identity
from scripts.benchmark_process import measure_process


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument(
        "--profile", choices=("default", "cache", "sustained", "all"), default="default"
    )
    result.add_argument("--hours", type=int, choices=(1, 2), default=1)
    result.add_argument(
        "--output", type=Path, default=ROOT / "target/benchmark-results"
    )
    result.add_argument(
        "--backend", choices=("auto", "cpu", "hardware-wgpu"), default="auto"
    )
    result.add_argument("--graphics-api", choices=("vulkan", "dx12", "metal", "gl"))
    result.add_argument(
        "--only",
        nargs="+",
        choices=("cpu", "moviepy", "cache", "sustained"),
        help="select tasks within the profile",
    )
    result.add_argument(
        "--suite",
        default="community-v1",
        choices=tuple(
            json.loads((ROOT / "benchmarks/suites.json").read_text())["suites"]
        ),
    )
    result.add_argument(
        "--smoke",
        action="store_true",
        help="small correctness runs; sustained duration becomes 3 seconds",
    )
    result.add_argument(
        "--timeout",
        type=float,
        default=900,
        help="per short suite/worker timeout in seconds",
    )
    result.add_argument("--build-timeout", type=float, default=7200)
    result.add_argument("--sustained-timeout", type=float, default=14400)
    result.add_argument(
        "--assets", type=Path, help="optional local MoviePy footage collection"
    )
    result.add_argument(
        "--retain-media",
        action="store_true",
        help="retain completed media locally; ZIP always excludes it",
    )
    result.add_argument("--preflight-only", action="store_true")
    result.add_argument(
        "--worker",
        choices=("build", "metadata", "cache-moviepy", "cache-placements", "sustained"),
        help=argparse.SUPPRESS,
    )
    result.add_argument("--request", type=Path, help=argparse.SUPPRESS)
    result.add_argument("--result", type=Path, help=argparse.SUPPRESS)
    result.add_argument("--prepared-directory", type=Path, help=argparse.SUPPRESS)
    return result


def selected(args):
    choices = {
        "default": {"cpu", "moviepy"},
        "cache": {"cache"},
        "sustained": {"sustained"},
        "all": {"cpu", "moviepy", "cache", "sustained"},
    }[args.profile]
    if args.only:
        if not set(args.only) <= choices:
            raise ValueError("--only must select tasks from the requested profile")
        choices &= set(args.only)
    return choices


def build_target(package, target, kind="example", features=None):
    command = [
        "cargo",
        "build",
        "--release",
        "--locked",
        "-p",
        package,
        f"--{kind}",
        target,
        "--message-format=json",
    ]
    if features:
        command += ["--features", features]
    result = subprocess.run(
        command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True
    )
    for line in result.stdout.splitlines():
        item = json.loads(line)
        if (
            item.get("reason") == "compiler-artifact"
            and item.get("target", {}).get("name") == target
            and item.get("executable")
        ):
            return item["executable"]
    raise ValueError(f"Cargo did not report {target}")


def worker(args):
    request = json.loads(args.request.read_text()) if args.request else {}
    if args.worker == "metadata":
        from benchmarks.comparisons.moviepy.comparison import environment

        data = environment()
        data.update(
            uv=host.version(["uv", "--version"]),
            cargo=host.version(["cargo", "--version"]),
        )
        write_json(args.result, data)
    elif args.worker == "build":
        if request["target"] == "benchmark":
            from scripts.benchmark import build_benchmark

            executable = str(build_benchmark(request["backend"]))
        elif request["target"] == "adapters":
            executable = build_target("vestra-render", "wgpu_adapters", features="wgpu")
        else:
            executable = build_target("vestra-media", "benchmark_access")
        write_json(
            args.result, dict(executable=executable, sha256=digest(Path(executable)))
        )
    elif args.worker == "cache-moviepy":
        from benchmarks.collector.cache import moviepy_access

        moviepy_access(request, args.result)
    elif args.worker == "cache-placements":
        from benchmarks.collector.cache import placement_worker

        placement_worker(request, args.result)
    else:
        from benchmarks.collector.sustained import worker as sustained_worker

        sustained_worker(request, args.result)


def hardware_candidates(text):
    """Consume Vestra's authoritative discovery classification, not vendor guesses."""
    import re

    rows = []
    for line in text.splitlines():
        backend = re.search(r"\bbackend=(\w+)", line)
        classification = re.search(r"\bclassification=(\w+)", line)
        if backend and classification:
            rows.append(
                dict(
                    backend=backend[1],
                    classification=classification[1],
                    raw=line,
                    eligible=classification[1] in {"integrated_gpu", "discrete_gpu"},
                )
            )
    return rows


class Collection:
    def __init__(self, args):
        self.args = args
        self.directory = args.prepared_directory or reporting.new_directory(
            args.output.resolve(), platform.system()
        )
        self.diagnostics = self.directory / "diagnostics"
        self.diagnostics.mkdir(exist_ok=True)
        self.python = sys.executable
        self.environment = os.environ.copy()
        # Exclude inherited tracing/strict/fallback controls that can invalidate
        # requested backend measurements. Explicit graphics API is retained.
        for key in list(self.environment):
            if key.startswith("VESTRA_BENCH_") or key in {
                "VESTRA_REQUIRE_WGPU",
                "VESTRA_REQUIRE_HARDWARE_WGPU",
                "VESTRA_WGPU_FORCE_FALLBACK",
                "RUST_LOG",
                "VESTRA_CPU_PROFILE",
            }:
                self.environment.pop(key)
        if args.graphics_api:
            self.environment["VESTRA_WGPU_BACKEND"] = args.graphics_api
        self.environment["VESTRA_BENCH_VALIDATE"] = "1"
        self.system = dict(
            platform=f"{platform.system()} {platform.release()}",
            machine=platform.machine(),
            logical_cpus=os.cpu_count(),
            python=sys.version,
            uv=host.version(["uv", "--version"]),
            cpu=platform.processor(),
            memory_bytes=host.physical_memory(),
        )
        self.manifest = dict(
            collection_schema_version=1,
            status="incomplete",
            profile=args.profile,
            smoke=args.smoke,
            settings=vars(args).copy(),
            tasks=[],
            started_at=datetime.now(timezone.utc).isoformat(),
            revision=host.version(["git", "rev-parse", "HEAD"]),
            # Git itself is a prerequisite; do not invoke it before preflight.
            source_sha256=None,
            suite_definitions=json.loads((ROOT / "benchmarks/suites.json").read_text()),
        )
        self.manifest["settings"] = {
            k: str(v) if isinstance(v, Path) else v
            for k, v in self.manifest["settings"].items()
            if k not in {"worker", "request", "result"}
        }
        if args.prepared_directory:
            self.manifest = json.loads((self.directory / "manifest.json").read_text())
            self.system = json.loads((self.directory / "system.json").read_text())

    def save(self):
        write_json(self.directory / "manifest.json", self.manifest)

    def execute(self, command, log, *, timeout=None, env=None, **kwargs):
        measurement = measure_process(
            command,
            log,
            timeout=timeout or self.args.timeout,
            env=env or self.environment,
            **kwargs,
        )
        write_json(Path(log).with_suffix(".measurement.json"), measurement)
        if (
            measurement["returncode"]
            or measurement["timed_out"]
            or measurement["disk_exhausted"]
        ):
            raise ValueError(
                f"command failed: {command}; exit={measurement['returncode']}; timeout={measurement['timed_out']}; disk_exhausted={measurement['disk_exhausted']}; see {Path(log).name}"
            )
        return measurement

    def task(self, name, function, *, validation="unavailable"):
        row = dict(
            name=name,
            status="incomplete",
            validation="not executed",
            validation_method=validation,
        )
        self.manifest["tasks"].append(row)
        self.save()
        print(f"Starting {name}", flush=True)
        try:
            result = function()
            row.update(
                status="passed",
                validation="passed" if validation != "unavailable" else "unavailable",
            )
            self.save()
            return result
        except Exception as error:
            row.update(status="failed", validation="not proven", error=str(error))
            (self.diagnostics / f"{name}.log").write_text(
                traceback.format_exc(), encoding="utf-8"
            )
            self.save()
            print(f"{name}: failed; other independent tasks will continue", flush=True)
            return None
        except KeyboardInterrupt:
            row.update(status="incomplete", error="cancelled by user")
            self.save()
            raise

    def skip(self, name, reason, *, unsupported=False):
        self.manifest["tasks"].append(
            dict(
                name=name,
                status="unsupported" if unsupported else "skipped",
                reason=reason,
                validation="not executed",
            )
        )
        self.save()

    def build(self, name, target, backend="cpu"):
        request = self.diagnostics / f"{name}-request.json"
        result = self.diagnostics / f"{name}-build.local.json"
        write_json(request, dict(target=target, backend=backend))
        self.execute(
            [
                self.python,
                str(ROOT / "scripts/collect-benchmarks.py"),
                "--worker",
                "build",
                "--request",
                str(request),
                "--result",
                str(result),
            ],
            self.diagnostics / f"{name}-build.log",
            timeout=self.args.build_timeout,
        )
        data = json.loads(result.read_text())
        self.manifest.setdefault("executables", {})[name] = data
        return Path(data["executable"])

    def prepare_python(self):
        environment = host.managed_environment()
        # Install the locked build backend before invoking it in this same
        # managed environment. Native compilation remains after preflight.
        self.execute(
            host.sync_command(sys.executable, dependencies_only=True),
            self.diagnostics / "uv-dependencies.log",
            timeout=self.args.build_timeout,
            env=environment,
        )
        self.execute(
            host.sync_command(sys.executable),
            self.diagnostics / "uv-sync.log",
            timeout=self.args.build_timeout,
            env=environment,
        )
        result = subprocess.run(
            [
                "uv",
                "run",
                "--no-sync",
                "--python",
                sys.executable,
                "python",
                "-c",
                "import sys; print(sys.executable)",
            ],
            cwd=ROOT,
            env=environment,
            capture_output=True,
            check=True,
            text=True,
            timeout=30,
        )
        self.python = result.stdout.strip()
        self.execute(
            [
                self.python,
                str(ROOT / "scripts/collect-benchmarks.py"),
                "--worker",
                "metadata",
                "--result",
                str(self.directory / "system.json"),
            ],
            self.diagnostics / "metadata.log",
        )
        self.system = json.loads((self.directory / "system.json").read_text())
        return True

    def vestra(self, backend, executable, suite):
        destination = self.directory / "vestra" / backend / suite
        destination.parent.mkdir(parents=True, exist_ok=True)
        scratch = self.directory / "local/native-bench-tmp"
        scratch.mkdir(parents=True, exist_ok=True)
        environment = {
            **self.environment,
            **{key: str(scratch.resolve()) for key in ("TMPDIR", "TMP", "TEMP")},
        }
        try:
            self.execute(
                [
                    self.python,
                    str(ROOT / "scripts/benchmark.py"),
                    "run",
                    "--suite",
                    suite,
                    "--backend",
                    backend,
                    "--executable",
                    str(executable),
                    "--output",
                    str(destination),
                ],
                self.diagnostics / f"{backend}-{suite}.log",
                env=environment,
            )
        finally:
            # SIGKILL bypasses Rust tempfile destructors. Only this invocation's
            # private scratch is eligible, after the owned worker tree is reaped.
            shutil.rmtree(scratch)
        data = json.loads((destination / "suite.json").read_text())
        if not all(
            r.get("validation", {}).get("status") == "passed" for r in data["records"]
        ):
            raise ValueError("missing benchmark output validation")
        return data

    def discover(self):
        executable = self.build("adapters", "adapters")
        log = self.diagnostics / "adapters.log"
        self.execute([str(executable)], log)
        rows = hardware_candidates(log.read_text())
        self.manifest["adapters"] = rows
        candidate = next(
            (
                r
                for r in rows
                if r["eligible"]
                and (
                    not self.args.graphics_api or r["backend"] == self.args.graphics_api
                )
            ),
            None,
        )
        if candidate:
            self.environment["VESTRA_WGPU_BACKEND"] = candidate["backend"]
        return candidate or False

    def moviepy(self, engine, inputs):
        destination = self.directory / "moviepy" / engine
        destination.parent.mkdir(exist_ok=True)
        self.execute(
            [
                self.python,
                str(ROOT / "scripts/benchmark.py"),
                "moviepy",
                "run",
                "--inputs",
                str(inputs),
                "--engines",
                engine,
                "--width",
                "640",
                "--height",
                "360",
                "--warmups",
                "0" if self.args.smoke else "1",
                "--samples",
                "1" if self.args.smoke else "3",
                "--timeout",
                str(self.args.timeout),
                "--output",
                str(destination),
            ],
            self.diagnostics / f"moviepy-{engine}.log",
            timeout=self.args.timeout * (3 if self.args.smoke else 12) + 300,
        )
        return destination

    def run(self):
        args = self.args
        chosen = selected(args)
        if not args.prepared_directory:
            preflight = host.preflight(self.directory)
            write_json(self.directory / "preflight.json", preflight)
            print("Vestra benchmark preflight", flush=True)
            for item in preflight["checks"]:
                print(
                    f"{item['name']:<26} {item['status'].upper()} {item.get('guidance', '')}",
                    flush=True,
                )
            self.manifest["tasks"].append(
                dict(
                    name="preflight",
                    status=preflight["status"],
                    validation="passed"
                    if preflight["status"] == "passed"
                    else "failed",
                )
            )
            if preflight["status"] != "passed":
                print(
                    "No benchmark measurements were started. Install missing native tools and retry.",
                    flush=True,
                )
                return
            self.manifest["source_sha256"] = source_identity()
            if args.preflight_only:
                return
            ready = self.task("python-environment", self.prepare_python)
            if ready:
                # Re-enter with the exact uv-managed interpreter so the controller
                # itself has psutil. The stdlib bootstrap must not import site
                # packages from an unrelated environment or pretend missing RSS
                # is a successful resource measurement.
                os.execve(
                    self.python,
                    [
                        self.python,
                        str(ROOT / "scripts/collect-benchmarks.py"),
                        *sys.argv[1:],
                        "--prepared-directory",
                        str(self.directory),
                    ],
                    self.environment,
                )
        else:
            ready = True
        cpu = None
        if "cpu" in chosen:
            cpu = self.task("cpu-build", lambda: self.build("cpu", "benchmark"))
            if cpu:
                self.task(
                    "cpu-smoke",
                    lambda: self.vestra("cpu", cpu, "smoke"),
                    validation="published outputs probed and fully decoded",
                )
                if not args.smoke:
                    self.task(
                        "cpu-suite",
                        lambda: self.vestra("cpu", cpu, args.suite),
                        validation="published outputs probed and fully decoded",
                    )
        gpu = None
        if args.backend != "cpu" and chosen & {"cpu", "moviepy", "sustained"}:
            candidate = self.task("gpu-discovery", self.discover)
            if candidate:
                if "cpu" in chosen:
                    gpu = self.task(
                        "wgpu-build",
                        lambda: self.build("wgpu", "benchmark", "hardware-wgpu"),
                    )
                    if gpu:
                        self.task(
                            "wgpu-suite",
                            lambda: self.vestra(
                                "hardware-wgpu",
                                gpu,
                                "smoke" if args.smoke else args.suite,
                            ),
                            validation="strict hardware backend; published outputs decoded",
                        )
            elif candidate is False:
                self.skip(
                    "hardware-wgpu",
                    "No eligible adapter from Vestra discovery for the selected graphics API; software/virtual/unknown adapters are ineligible.",
                    unsupported=True,
                )
                if args.backend == "hardware-wgpu":
                    self.manifest["tasks"][-1]["status"] = "failed"
        else:
            candidate = False
            self.skip(
                "hardware-wgpu",
                "CPU-only collection or native decoder cache profile requested",
            )
        if not ready:
            for name in chosen - {"cpu"}:
                self.skip(name, "managed Python environment preparation failed")
            return
        inputs = self.directory / "local" / "inputs"
        inputs.parent.mkdir(exist_ok=True)
        if chosen & {"moviepy", "cache", "sustained"}:

            def prepare_inputs():
                if args.assets:
                    from benchmarks.comparisons.moviepy.comparison import prepare

                    prepare(argparse.Namespace(assets=args.assets, inputs=inputs))
                else:
                    from benchmarks.collector.fixtures import portable_inputs

                    portable_inputs(inputs)
                shutil.copy2(
                    inputs / "manifest.json", self.directory / "input-manifest.json"
                )
                return True

            prepared = self.task(
                "input-fixtures",
                prepare_inputs,
                validation="hashes, frame counts, full decode",
            )
            if not prepared:
                for name in chosen - {"cpu"}:
                    self.skip(name, "input preparation failed")
                return
        if "moviepy" in chosen:
            captures = []
            for engine in ["vestra-cpu", "moviepy-cpu"] + (
                ["hardware-wgpu"] if candidate else []
            ):
                capture = self.task(
                    f"moviepy-{engine}",
                    lambda e=engine: self.moviepy(e, inputs),
                    validation="structural/full-decode; visual check is separate",
                )
                if capture:
                    captures.append(capture)
            if len(captures) >= 2:

                def verify():
                    self.execute(
                        [
                            self.python,
                            str(ROOT / "scripts/benchmark.py"),
                            "moviepy",
                            "verify",
                            *map(str, captures),
                            "--inputs",
                            str(inputs),
                            "--output",
                            str(self.directory / "moviepy/verification"),
                        ],
                        self.diagnostics / "visual-verification.log",
                        timeout=max(args.timeout, 300),
                    )

                self.task(
                    "moviepy-visual",
                    verify,
                    validation="decoded boundary/source frames and layer regions",
                )
            else:
                self.skip(
                    "moviepy-visual", "requires at least two successful engine captures"
                )
        if chosen & {"cache", "sustained"}:
            long_inputs = inputs.parent / "long-inputs" if args.smoke else inputs
            source = long_inputs / "long.mp4"

            def prepare_long():
                from benchmarks.collector.fixtures import long_source, portable_inputs

                if args.smoke:
                    portable_inputs(long_inputs, width=128, height=72)
                    shutil.copy2(
                        long_inputs / "manifest.json",
                        self.directory / "long-input-manifest.json",
                    )
                settings = json.loads((long_inputs / "manifest.json").read_text())[
                    "settings"
                ]
                result = long_source(
                    long_inputs / "forest.mp4",
                    source,
                    width=settings["width"],
                    height=settings["height"],
                )
                write_json(self.directory / "long-source.json", result)
                return True

            long_ready = self.task(
                "long-source",
                prepare_long,
                validation="all PTS and early/middle/end seek pixels",
            )
            if not long_ready:
                for name in chosen & {"cache", "sustained"}:
                    self.skip(name, "long-source validation failed")
            if long_ready and "cache" in chosen:
                native = self.task("cache-build", lambda: self.build("cache", "access"))
                if native:
                    from benchmarks.collector.cache import run

                    self.task(
                        "cache",
                        lambda: run(
                            self.directory / "cache",
                            source,
                            native,
                            self.execute,
                            self.python,
                            samples=1 if args.smoke else 3,
                            warmups=0 if args.smoke else 1,
                            smoke=args.smoke,
                            timeout=args.timeout,
                        ),
                        validation="exact native cache-on/off RGBA equality for every request",
                    )
                else:
                    self.skip("cache", "native decoder harness build failed")
            if long_ready and "sustained" in chosen:
                from benchmarks.collector.sustained import run

                sustained_inputs = inputs.parent / "sustained-inputs"
                sustained_inputs.mkdir()
                try:
                    os.link(source, sustained_inputs / "forest.mp4")
                except OSError:
                    shutil.copy2(source, sustained_inputs / "forest.mp4")
                self.task(
                    "sustained",
                    lambda: run(
                        self.directory / "sustained",
                        sustained_inputs,
                        self.execute,
                        self.python,
                        ["vestra-cpu", "moviepy-cpu"]
                        + (["hardware-wgpu"] if candidate else []),
                        duration=3 if args.smoke else args.hours * 3600,
                        smoke=args.smoke,
                        timeout=args.sustained_timeout,
                        retain=args.retain_media,
                    ),
                    validation="duration/count/full decode and early/middle/end frames",
                )

    def finish(self, cancelled=False):
        tasks = self.manifest["tasks"]
        if (
            not cancelled
            and not self.args.preflight_only
            and self.manifest["source_sha256"] is not None
            and self.manifest["source_sha256"] != source_identity()
        ):
            tasks.append(
                dict(
                    name="source-stability",
                    status="failed",
                    error="source changed during collection; captured measurements are invalid",
                )
            )
        failed = any(t["status"] in {"failed", "incomplete"} for t in tasks)
        self.manifest.update(
            status="incomplete" if cancelled else "failed" if failed else "passed",
            finished_at=datetime.now(timezone.utc).isoformat(),
        )
        self.save()
        write_json(self.directory / "system.json", self.system)
        reporting.summarize(self.directory, self.manifest, self.system)
        archive = reporting.archive(
            self.directory,
            private_roots=[
                self.directory,
                *([self.args.assets.resolve()] if self.args.assets else []),
            ],
        )
        retained = set()
        if self.args.retain_media:
            for report, data in reporting.documents(
                self.directory.glob("moviepy/**/comparison.json")
            ):
                for row in data["records"]:
                    for sample in row["samples"] + row["warmups"]:
                        if sample.get("validation", {}).get("status") == "ok":
                            retained.add(report.parent / sample["output"])
            for report, data in reporting.documents(
                self.directory.glob("sustained/*/result.json")
            ):
                if data.get("status") == "passed":
                    retained.add(report.parent / "local/render.mp4")
            # Prepared inputs were validated before execution. Preserve those
            # explicitly, but never incomplete generation/publication targets.
            for name in ("inputs", "long-inputs"):
                inputs = self.directory / "local" / name
                if (inputs / "manifest.json").is_file():
                    from benchmarks.comparisons.moviepy.comparison import load_inputs

                    try:
                        prepared = load_inputs(inputs)
                        retained.update(
                            inputs / item["filename"]
                            for item in prepared["prepared"].values()
                        )
                    except ValueError:
                        pass
                if (inputs / "long.fixture.json").is_file():
                    retained.add(inputs / "long.mp4")
        for path in self.directory.rglob("*.mp4"):
            if path not in retained:
                path.unlink()
        print(f"\nBenchmark collection {self.manifest['status']}\n", flush=True)
        for task in tasks:
            print(f"{task['name']:<28} {task['status'].upper()}", flush=True)
        print(
            f"\nArchive:\n{archive}\n\nSend this ZIP archive to the maintainer.",
            flush=True,
        )
        return 130 if cancelled else 1 if failed else 0


def main(argv=None):
    cli = parser()
    args = cli.parse_args(argv)
    if args.worker:
        if not args.result:
            cli.error("worker requires --result")
        worker(args)
        return 0
    try:
        selected(args)
        if any(
            not math.isfinite(value) or value <= 0
            for value in (args.timeout, args.build_timeout, args.sustained_timeout)
        ):
            raise ValueError("timeouts must be finite and positive")
    except ValueError as error:
        cli.error(str(error))
    collection = Collection(args)
    cancelled = False
    try:
        collection.run()
    except KeyboardInterrupt:
        cancelled = True
    except Exception as error:
        collection.manifest["tasks"].append(
            dict(name="collection", status="failed", error=str(error))
        )
        (collection.diagnostics / "collection.log").write_text(
            traceback.format_exc(), encoding="utf-8"
        )
    return collection.finish(cancelled)
