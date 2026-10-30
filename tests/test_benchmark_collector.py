"""Portable contracts; expensive native rendering is an explicit integration gate."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from benchmarks.collector import cache, collect, fixtures, host, reporting, sustained  # noqa: E402
from benchmarks.comparisons.moviepy.comparison import load_inputs  # noqa: E402
from scripts.benchmark import require_backend, suite_configuration  # noqa: E402
from scripts.benchmark_process import measure_process  # noqa: E402


@pytest.mark.parametrize(
    "profile,expected",
    [
        ("default", {"cpu", "moviepy"}),
        ("cache", {"cache"}),
        ("sustained", {"sustained"}),
        ("all", {"cpu", "moviepy", "cache", "sustained"}),
    ],
)
def test_profiles(profile, expected):
    args = collect.parser().parse_args(["--profile", profile])
    assert collect.selected(args) == expected
    assert not args.smoke


def test_selection_and_invalid_hours():
    args = collect.parser().parse_args(["--only", "cpu"])
    assert collect.selected(args) == {"cpu"}
    args.only = ["sustained"]
    with pytest.raises(ValueError):
        collect.selected(args)
    with pytest.raises(SystemExit):
        collect.parser().parse_args(["--hours", "3"])


def test_bootstrap_has_no_native_or_moviepy_import():
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import runpy, sys; sys.argv=['collect-benchmarks.py','--help']; runpy.run_path('scripts/collect-benchmarks.py',run_name='__main__')",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0
    assert "--profile" in result.stdout
    source = (ROOT / "scripts/collect-benchmarks.py").read_text()
    assert "dependencies = []" in source


def test_uv_locked_environment_isolation():
    command = host.sync_command("C:/Python/python.exe")
    assert command[:3] == ["uv", "sync", "--locked"]
    assert command[command.index("--extra") + 1] == "benchmark"
    assert command[command.index("--python") + 1] == "C:/Python/python.exe"
    assert "--reinstall-package" in command
    assert "--no-editable" in command and "--no-cache" in command
    assert command[command.index("--no-build-isolation-package") + 1] == "vestra"
    assert "build-args=--profile release --locked" in command
    assert host.managed_environment()["UV_PROJECT_ENVIRONMENT"].endswith(
        "target/benchmark-env"
    )
    import tomllib

    project = tomllib.loads((ROOT / "pyproject.toml").read_text())
    assert "moviepy==2.2.1" in project["project"]["optional-dependencies"]["benchmark"]
    lock = tomllib.loads((ROOT / "uv.lock").read_text())
    build_tool = next(p for p in lock["package"] if p["name"] == "maturin")
    assert (
        f"maturin=={build_tool['version']}"
        in project["project"]["optional-dependencies"]["benchmark"]
    )
    dependencies = host.sync_command("managed-python", dependencies_only=True)
    assert "--locked" in dependencies and "--no-install-project" in dependencies
    assert "--config-settings" not in dependencies
    assert "--no-cache" not in dependencies
    assert project["tool"]["uv"]["build-constraint-dependencies"] == [
        f"maturin=={build_tool['version']}"
    ]


def test_managed_environment_does_not_inherit_bootstrap_venv(monkeypatch):
    monkeypatch.setenv("VIRTUAL_ENV", "/bootstrap-environment")
    assert "VIRTUAL_ENV" not in host.managed_environment()


@pytest.mark.parametrize(
    "system,name", [("Windows", "tool.exe"), ("Darwin", "tool"), ("Linux", "tool")]
)
def test_executable_resolution(monkeypatch, system, name):
    monkeypatch.setattr(
        host.shutil, "which", lambda n: f"/tools/{name}" if n == name else None
    )
    assert host.executable("tool", system) == f"/tools/{name}"


@pytest.mark.parametrize("system", ["Windows", "Darwin", "Linux"])
def test_missing_native_dependencies(monkeypatch, tmp_path, system):
    monkeypatch.setattr(host.platform, "system", lambda: system)
    monkeypatch.setattr(host.platform, "machine", lambda: "AMD64")
    monkeypatch.setattr(host, "version", lambda command: None)
    monkeypatch.setattr(host, "find_libclang", lambda system: None)
    monkeypatch.setattr(host, "codec_check", lambda: True)
    monkeypatch.setattr(
        host,
        "compiler_check",
        lambda *a: (_ for _ in ()).throw(ValueError("compiler missing")),
    )
    monkeypatch.setattr(host, "msvc_host", lambda: False)
    monkeypatch.setattr(host, "shell_tools", lambda: False)
    report = host.preflight(tmp_path)
    assert report["status"] == "failed"
    rows = {r["name"]: r for r in report["checks"]}
    assert rows["cargo"]["status"] == "failed"
    assert "COLLECTING" in rows["cargo"]["guidance"]
    assert (
        "MSVC" in rows["C/C++ compiler and linker"]["guidance"]
        or "Windows" in rows["C/C++ compiler and linker"]["guidance"]
    )
    if system == "Windows":
        assert "sh" in rows
    if system == "Linux":
        assert "pkg-config" in rows
    assert not any("FFmpeg dev" in name for name in rows)


def test_future_prebuilt_mode_omits_build_tools(monkeypatch, tmp_path):
    monkeypatch.setattr(host, "version", lambda command: "test version")
    monkeypatch.setattr(host, "codec_check", lambda: True)
    report = host.preflight(tmp_path, build=False)
    assert not {"cargo", "libclang", "make"} & {r["name"] for r in report["checks"]}


def test_directory_collision(monkeypatch, tmp_path):
    instant = reporting.datetime.now(reporting.timezone.utc)
    monkeypatch.setattr(
        reporting, "datetime", argparse.Namespace(now=lambda _: instant)
    )
    monkeypatch.setattr(
        reporting.uuid, "uuid4", lambda: argparse.Namespace(hex="12345678")
    )
    first = reporting.new_directory(tmp_path, "Windows")
    with pytest.raises(FileExistsError):
        reporting.new_directory(tmp_path, "Windows")
    assert first.is_dir()


def test_missing_git_preflight_still_produces_archive(monkeypatch, tmp_path):
    monkeypatch.setattr(
        collect,
        "source_identity",
        lambda: (_ for _ in ()).throw(AssertionError("Git invoked before preflight")),
    )
    monkeypatch.setattr(
        host,
        "preflight",
        lambda directory: dict(
            status="failed",
            checks=[dict(name="git", status="failed", guidance="install Git")],
        ),
    )
    assert collect.main(["--output", str(tmp_path)]) == 1
    (archive,) = tmp_path.glob("*.zip")
    with zipfile.ZipFile(archive) as bundle:
        manifest = json.loads(
            bundle.read(
                next(n for n in bundle.namelist() if n.endswith("/manifest.json"))
            )
        )
        assert manifest["status"] == "failed"
        assert manifest["tasks"] == [
            dict(name="preflight", status="failed", validation="failed")
        ]


def test_community_suite_preserves_canonical():
    manifest = json.loads((ROOT / "benchmarks/suites.json").read_text())
    settings, names = suite_configuration(manifest, "community-v1")
    assert settings["samples"] == 3 and settings["warmups"] == 1
    assert {
        "single_video",
        "mixed_dynamic",
        "video_heavy",
        "masks",
        "mattes",
        "production_edit",
    } == set(names)
    assert suite_configuration(manifest, "canonical")[0] == dict(
        width=1280, height=720, warmups=1, samples=5
    )


def test_moviepy_smoke_keeps_comparison_canvas(monkeypatch, tmp_path):
    args = collect.parser().parse_args(["--output", str(tmp_path), "--smoke"])
    collection = collect.Collection(args)
    commands = []
    monkeypatch.setattr(
        collection, "execute", lambda command, *a, **k: commands.append(command)
    )
    collection.moviepy("vestra-cpu", tmp_path / "inputs")
    command = commands[0]
    assert command[command.index("--width") + 1] == "640"
    assert command[command.index("--height") + 1] == "360"
    assert command[command.index("--samples") + 1] == "1"
    assert command[command.index("--warmups") + 1] == "0"


@pytest.mark.parametrize(
    "classification,eligible",
    [
        ("software", False),
        ("cpu", False),
        ("virtual_gpu", False),
        ("unknown", False),
        ("discrete_gpu", True),
        ("integrated_gpu", True),
    ],
)
def test_hardware_classification(classification, eligible):
    rows = collect.hardware_candidates(
        f'backend=metal adapter="GPU" classification={classification} driver="x"'
    )
    assert rows[0]["eligible"] == eligible
    result = dict(render_backend="wgpu", adapter=dict(performance_class=classification))
    if eligible:
        require_backend(result, "hardware-wgpu")
    else:
        with pytest.raises(ValueError):
            require_backend(result, "hardware-wgpu")


def test_portable_generation_and_hash_gate(monkeypatch, tmp_path):
    calls = []

    def execute(args, timeout=300):
        calls.append(args)
        Path(args[-1]).write_bytes(b"moving-video")

    monkeypatch.setattr(fixtures, "command", execute)
    monkeypatch.setattr(fixtures, "validate_output", lambda *a: dict(status="ok"))
    monkeypatch.setattr(fixtures, "version", lambda *a: "ffmpeg test")
    inputs = tmp_path / "inputs"
    fixtures.portable_inputs(inputs, width=128, height=72)
    manifest = load_inputs(inputs)
    assert len(manifest["prepared"]) == 3
    assert all("testsrc2=" in " ".join(map(str, command)) for command in calls)
    (inputs / "forest.mp4").write_bytes(b"changed")
    with pytest.raises(ValueError, match="changed"):
        load_inputs(inputs)


def test_cache_patterns_are_reproducible_and_evict():
    sequences = cache.patterns()
    assert sequences == cache.patterns()
    assert len(set(r["seconds"] for r in sequences["eviction"])) > 8
    assert (
        sequences["local_reuse"][4]["seconds"] < sequences["local_reuse"][3]["seconds"]
    )
    assert max(r["seconds"] for r in sequences["random_seeking"]) > 3600
    assert {r["session"] for r in sequences["repeated_placements"]} == {0, 1}
    assert len({r["seconds"] for r in sequences["repeated_frame"]}) == 1
    placement = cache.placement_definition(640, 360)
    assert {c["asset"] for c in placement["clips"]} == {"long"}
    assert {c["source_start"] for c in placement["clips"]} == {0, 6}


def test_moviepy_clip_opens_are_not_native_decoder_counters(monkeypatch, tmp_path):
    clip = argparse.Namespace(get_frame=lambda seconds: b"pixels", close=lambda: None)
    monkeypatch.setitem(
        sys.modules,
        "moviepy",
        argparse.Namespace(VideoFileClip=lambda *a, **k: clip),
    )
    result = tmp_path / "result.json"
    cache.moviepy_access(
        dict(
            source="source.mp4",
            sessions=1,
            verify=False,
            requests=[dict(session=0, seconds=100)],
        ),
        result,
    )
    data = json.loads(result.read_text())
    assert data["decoder_opens"] is None
    assert data["initial_clip_opens"] == 1


def test_cache_rejects_missing_exact_equivalence(tmp_path):
    source = tmp_path / "source.mp4"
    source.write_bytes(b"source")
    source.with_suffix(".fixture.json").write_text(
        json.dumps(dict(width=128, height=72, duration=7200))
    )

    def execute(command, log, **kwargs):
        Path(command[-1]).write_text(json.dumps(dict(exact_verified_frames=0)))
        return {}

    with pytest.raises(ValueError, match="exact"):
        cache.run(
            tmp_path / "cache",
            source,
            Path("native"),
            execute,
            sys.executable,
            smoke=True,
        )
    result = json.loads((tmp_path / "cache/results.json").read_text())
    assert result["status"] != "passed"


def test_resource_sampling_and_timeout(tmp_path):
    pytest.importorskip("psutil")
    (tmp_path / "progress.json").write_text('{"frame": 3, "total_frames": 90}')
    sample = measure_process(
        [sys.executable, "-c", "import time; a=bytearray(1024*1024); time.sleep(.15)"],
        tmp_path / "worker.log",
        timeout=5,
        series_path=tmp_path / "resources.jsonl",
        progress_path=tmp_path / "progress.json",
    )
    assert sample["returncode"] == 0 and sample["wall_ms"] > 100
    assert sample["peak_tree_rss_bytes"] > 0
    rows = [
        json.loads(line)
        for line in (tmp_path / "resources.jsonl").read_text().splitlines()
    ]
    assert rows[0]["elapsed_seconds"] >= 0
    assert all(row["cpu_percent"] >= 0 for row in rows)
    assert all(row["average_fps"] > 0 for row in rows)
    sample = measure_process(
        [sys.executable, "-c", "import time; time.sleep(10)"],
        tmp_path / "timeout.log",
        timeout=0.1,
    )
    assert sample["timed_out"] and sample["returncode"] != 0


def test_partial_failure_and_zip(monkeypatch, tmp_path):
    args = collect.parser().parse_args(["--output", str(tmp_path)])
    collection = collect.Collection(args)
    collection.task("cpu", lambda: True)
    collection.task("gpu", lambda: (_ for _ in ()).throw(ValueError("adapter error")))
    assert collection.finish() == 1
    assert collection.manifest["tasks"][0]["status"] == "passed"
    with zipfile.ZipFile(collection.directory.with_suffix(".zip")) as archive:
        assert archive.testzip() is None
        assert any(n.endswith("summary.md") for n in archive.namelist())
        report = json.loads(archive.read(collection.directory.name + "/manifest.json"))
        assert report["status"] == "failed"


def test_archive_privacy_and_media_exclusion(tmp_path):
    data = dict(
        path=str(Path.home() / "secret footage.mp4"),
        windows="C:\\Users\\Alice\\private.mp4",
        variables={"VESTRA_TOKEN": "must-not-share", "VESTRA_WGPU_BACKEND": "metal"},
        tags=dict(comment="private"),
        token="must-not-share",
    )
    (tmp_path / "record.json").write_text(json.dumps(data))
    (tmp_path / "log.log").write_text("API_KEY=must-not-share /Users/Alice/movie.mp4")
    (tmp_path / "render.mp4").write_bytes(b"do not share")
    (tmp_path / "local").mkdir()
    (tmp_path / "local/private.json").write_text(json.dumps(data))
    archive = reporting.archive(tmp_path)
    try:
        with zipfile.ZipFile(archive) as bundle:
            text = "".join(bundle.read(n).decode() for n in bundle.namelist())
            assert "must-not-share" not in text and "Alice" not in text
            assert str(Path.home()) not in text
            assert "private.json" not in str(bundle.namelist())
            assert "render.mp4" not in str(bundle.namelist())
    finally:
        archive.unlink()


def test_native_cancellation_removes_only_owned_scratch(monkeypatch, tmp_path):
    args = collect.parser().parse_args(["--output", str(tmp_path)])
    collection = collect.Collection(args)
    sentinel = tmp_path / "unrelated.mp4"
    sentinel.write_bytes(b"keep")

    def cancelled(command, log, *, env):
        scratch = Path(env["TMPDIR"])
        assert env["TMP"] == env["TEMP"] == str(scratch)
        (scratch / "incomplete.mp4").write_bytes(b"partial")
        (scratch / "input.mkv").write_bytes(b"generated source")
        raise KeyboardInterrupt

    monkeypatch.setattr(collection, "execute", cancelled)
    with pytest.raises(KeyboardInterrupt):
        collection.vestra("cpu", Path("benchmark"), "smoke")
    assert not (collection.directory / "local/native-bench-tmp").exists()
    assert sentinel.read_bytes() == b"keep"


def test_invalid_partial_json_is_packaged(tmp_path):
    (tmp_path / "partial.json").write_text('{"source": "/Users/Alice/private.mp4",')
    archive = reporting.archive(tmp_path)
    try:
        with zipfile.ZipFile(archive) as bundle:
            evidence = json.loads(bundle.read(tmp_path.name + "/partial.json"))
            assert evidence["status"] == "incomplete"
            assert "Alice" not in str(evidence)
    finally:
        archive.unlink()


def test_cancelled_collection_packages_truncated_worker_evidence(tmp_path):
    args = collect.parser().parse_args(["--output", str(tmp_path), "--retain-media"])
    collection = collect.Collection(args)
    collection.manifest["tasks"].append(dict(name="sustained", status="incomplete"))
    folder = collection.directory / "sustained/vestra-cpu"
    folder.mkdir(parents=True)
    (folder / "result.json").write_text(
        json.dumps(dict(status="incomplete", duration=3, frames=90))
    )
    (folder / "resources.jsonl").write_text(
        '{"tree_rss_bytes": 1234}\n{"path": "/Users/Alice/private.mp4",'
    )
    cache = collection.directory / "cache"
    cache.mkdir()
    (cache / "results.json").write_text('{"patterns":')
    moviepy = collection.directory / "moviepy/vestra-cpu"
    moviepy.mkdir(parents=True)
    (moviepy / "comparison.json").write_text('{"records":')
    (moviepy / "incomplete.mp4").write_bytes(b"partial")
    assert collection.finish(cancelled=True) == 130
    assert not (moviepy / "incomplete.mp4").exists()
    with zipfile.ZipFile(collection.directory.with_suffix(".zip")) as bundle:
        name = next(n for n in bundle.namelist() if n.endswith("resources.jsonl"))
        evidence = [json.loads(line) for line in bundle.read(name).splitlines()]
        assert evidence[0]["tree_rss_bytes"] == 1234
        assert evidence[1]["status"] == "incomplete"
        assert "Alice" not in str(evidence)


@pytest.mark.parametrize("value", ["nan", "inf", "0", "-1"])
def test_invalid_timeout_rejected_before_collection(value):
    with pytest.raises(SystemExit) as error:
        collect.main(["--timeout", value])
    assert error.value.code == 2


def test_native_libclang_probe_handles_python_abi_mismatch(monkeypatch):
    monkeypatch.setattr(
        host, "libclang_candidates", lambda system: iter(["native-libclang"])
    )
    monkeypatch.setattr(
        host.ctypes,
        "CDLL",
        lambda candidate: (_ for _ in ()).throw(OSError("different libc")),
    )
    monkeypatch.setattr(host, "native_library_check", lambda candidate, system: True)
    assert host.find_libclang("Linux") == "native-libclang"


@pytest.mark.skipif(
    not shutil.which("ffmpeg") or not shutil.which("ffprobe"),
    reason="FFmpeg fixture integration",
)
def test_real_portable_fixture_validation(tmp_path):
    inputs = tmp_path / "inputs"
    fixtures.portable_inputs(inputs, width=128, height=72)
    assert load_inputs(inputs)["prepared"]["forest"]["validation"]["frames"] == 360
    long = fixtures.long_source(
        inputs / "forest.mp4", inputs / "long.mp4", duration=24, width=128, height=72
    )
    assert long["frames"] == 720 and long["timestamp_count"] == 720
    assert all(item["matches_segment"] for item in long["coverage"])


def test_timeout_kills_descendants(tmp_path):
    psutil = pytest.importorskip("psutil")
    pidfile = tmp_path / "child.pid"
    code = "import subprocess, sys, time; from pathlib import Path; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(20)']); Path(sys.argv[1]).write_text(str(p.pid)); time.sleep(20)"
    sample = measure_process(
        [sys.executable, "-c", code, str(pidfile)], tmp_path / "tree.log", timeout=0.5
    )
    assert sample["timed_out"]
    child_pid = int(pidfile.read_text())
    try:
        assert psutil.Process(child_pid).status() == psutil.STATUS_ZOMBIE
    except psutil.NoSuchProcess:
        pass


@pytest.mark.parametrize("duration,frames", [(3600, 108000), (7200, 216000), (3, 90)])
def test_sustained_frame_counts(duration, frames):
    work = sustained.definition(duration)
    assert work["duration"] * work["fps"] == frames
    assert all(c["duration"] == duration for c in work["clips"])
    assert sustained.disk_requirement(duration, 640, 360, 1) > 2**30


def test_sustained_summary_does_not_reuse_sparse_samples(tmp_path):
    folder = tmp_path / "sustained/vestra-cpu"
    folder.mkdir(parents=True)
    (folder / "result.json").write_text(
        json.dumps(
            dict(
                status="passed",
                duration=3,
                frames=90,
                measurement=dict(wall_ms=9000),
            )
        )
    )
    (folder / "resources.jsonl").write_text(
        "\n".join(
            json.dumps(dict(elapsed_seconds=elapsed, tree_rss_bytes=rss * 2**20))
            for elapsed, rss in [(0.2, 1), (8.5, 3)]
        )
    )
    reporting.summarize(
        tmp_path, dict(profile="all", smoke=True, status="passed", tasks=[]), {}
    )
    assert "1.000 / unavailable / 3.000" in (tmp_path / "summary.md").read_text()


def test_long_source_rejects_successful_probe_with_decode_errors(monkeypatch, tmp_path):
    source = tmp_path / "segment.mp4"
    source.write_bytes(b"segment")

    def command(args, timeout=300):
        if args[0] == "ffprobe":
            return json.dumps(
                dict(streams=[dict(codec_type="video", duration="24", nb_frames="720")])
            ).encode()
        Path(args[-1]).write_bytes(b"long")
        return b""

    def probe(args, *, stdout, **kwargs):
        stdout.write("\n".join(str(frame / 30) for frame in range(720)))
        return subprocess.CompletedProcess(args, 0, stderr=b"corrupt decoded frame")

    monkeypatch.setattr(fixtures, "command", command)
    monkeypatch.setattr(fixtures.subprocess, "run", probe)
    monkeypatch.setattr(fixtures, "frame_at", lambda *args: b"pixels")
    monkeypatch.setattr(fixtures, "version", lambda *args: "test")
    with pytest.raises(ValueError, match="corrupt decoded frame"):
        fixtures.long_source(source, tmp_path / "long.mp4", duration=24)


def test_sustained_final_stage_keeps_observed_progress(monkeypatch, tmp_path):
    def render(definition, inputs, output, backend, *, progress):
        for frame, total, kind in [
            (None, None, "started"),
            (90, 90, "progress"),
            (None, None, "completed"),
        ]:
            progress(
                argparse.Namespace(
                    frame=frame, total_frames=total, stage=None, kind=kind
                )
            )
        return dict(render_backend="cpu")

    monkeypatch.setattr(sustained, "render_vestra", render)
    progress = tmp_path / "progress.json"
    sustained.worker(
        dict(
            definition=sustained.definition(3),
            engine="vestra-cpu",
            progress=str(progress),
            inputs=str(tmp_path),
            media=str(tmp_path / "render.mp4"),
        ),
        tmp_path / "result.json",
    )
    data = json.loads(progress.read_text())
    assert data["frame"] == data["total_frames"] == 90
    assert data["kind"] == "completed"


def test_sustained_moviepy_progress_uses_v2_frame_bar(monkeypatch, tmp_path):
    monkeypatch.setitem(
        sys.modules, "proglog", argparse.Namespace(ProgressBarLogger=object)
    )

    def render(definition, inputs, output, *, progress):
        progress.bars_callback("frame_index", "index", 90)
        return dict(render_backend="cpu")

    monkeypatch.setattr(sustained, "render_moviepy", render)
    progress = tmp_path / "progress.json"
    sustained.worker(
        dict(
            definition=sustained.definition(3),
            engine="moviepy-cpu",
            progress=str(progress),
            inputs=str(tmp_path),
            media=str(tmp_path / "render.mp4"),
        ),
        tmp_path / "result.json",
    )
    data = json.loads(progress.read_text())
    assert data["frame"] == data["total_frames"] == 90


@pytest.mark.skipif(
    os.environ.get("VESTRA_COLLECTOR_E2E") != "1",
    reason="explicit native end-to-end collector smoke",
)
def test_real_collector_end_to_end(tmp_path):
    result = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts/collect-benchmarks.py"),
            "--profile",
            "all",
            "--smoke",
            "--backend",
            "cpu",
            "--output",
            str(tmp_path),
        ],
        cwd=ROOT,
        timeout=7200,
    )
    assert result.returncode == 0
    (archive,) = tmp_path.glob("*.zip")
    with zipfile.ZipFile(archive) as bundle:
        manifest = json.loads(
            bundle.read(
                next(n for n in bundle.namelist() if n.endswith("/manifest.json"))
            )
        )
        tasks = {t["name"]: t["status"] for t in manifest["tasks"]}
        assert {
            tasks[k] for k in ("cpu-smoke", "moviepy-visual", "cache", "sustained")
        } == {"passed"}
        for engine in ("vestra-cpu", "moviepy-cpu"):
            progress = json.loads(
                bundle.read(
                    next(
                        n
                        for n in bundle.namelist()
                        if n.endswith(f"/sustained/{engine}/progress.json")
                    )
                )
            )
            assert progress["frame"] == progress["total_frames"] == 90


@pytest.mark.parametrize("failed_metric", ["descriptors", "memory", "cpu"])
def test_resource_query_failures_are_independent(monkeypatch, tmp_path, failed_metric):
    psutil = pytest.importorskip("psutil")
    from types import SimpleNamespace

    class Member:
        pid = 12345

        def children(self, recursive):
            return []

        def query(self, metric, value):
            if metric == failed_metric:
                raise psutil.AccessDenied(self.pid, msg=f"{metric} denied")
            return value

        def memory_info(self):
            return self.query("memory", SimpleNamespace(rss=4096))

        def cpu_times(self):
            return self.query("cpu", SimpleNamespace(user=0.02, system=0.01))

        def create_time(self):
            return 1.0

        def num_fds(self):
            return self.query("descriptors", 7)

        num_handles = num_fds

    monkeypatch.setattr(psutil, "Process", lambda pid: Member())
    sample = measure_process(
        [sys.executable, "-c", "import time; time.sleep(.08)"],
        tmp_path / "worker.log",
        timeout=5,
        series_path=tmp_path / "resources.jsonl",
    )
    assert sample["returncode"] == 0
    assert sample["peak_tree_rss_bytes"] == (
        None if failed_metric == "memory" else 4096
    )
    assert sample["cpu_seconds"] == (None if failed_metric == "cpu" else 0.03)
    assert sample["measurement_complete"] == {
        metric: metric != failed_metric for metric in ("memory", "cpu", "descriptors")
    }
    assert sample["peak_handles_or_fds"] == (
        None if failed_metric == "descriptors" else 7
    )
    (error,) = sample["measurement_errors"]
    assert error["metric"] == failed_metric and error["pid"] == 12345
    assert error["error"] == "AccessDenied" and "denied" in error["message"]
    assert error["count"] > 0
    rows = [
        json.loads(line)
        for line in (tmp_path / "resources.jsonl").read_text().splitlines()
    ]
    assert all(
        row["tree_rss_bytes"] == (None if failed_metric == "memory" else 4096)
        for row in rows
    )
    assert all(
        row["cpu_seconds"] == (None if failed_metric == "cpu" else 0.03) for row in rows
    )
    assert all(
        row["handles_or_fds"] == (None if failed_metric == "descriptors" else 7)
        for row in rows
    )
