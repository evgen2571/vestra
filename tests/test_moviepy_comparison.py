"""Observable contracts for the external comparison (no native build required)."""

import importlib.util
import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import benchmark  # noqa: E402


def comparison():
    path = ROOT / "benchmarks/comparisons/moviepy/comparison.py"
    assert path.is_file(), "external comparison implementation is missing"
    spec = importlib.util.spec_from_file_location("moviepy_comparison", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_missing_assets_are_actionable(tmp_path):
    with pytest.raises(ValueError, match="forward-fly-above-snow-forest.mp4"):
        comparison().discover(tmp_path)


def test_workloads_have_exact_timelines():
    c = comparison()
    for name, frames in [("basic", 300), ("editing", 360), ("layered", 300)]:
        workload = c.workload(name, 1920, 1080)
        assert workload["duration"] * workload["fps"] == frames
        assert (
            max(x["start"] + x["duration"] for x in workload["clips"])
            == workload["duration"]
        )
        assert {x["asset"] for x in workload["clips"]} <= {"forest", "valley", "tear"}
    editing = c.workload("editing", 1920, 1080)["clips"]
    assert editing[1]["start"] == 3.8
    assert editing[1]["fade_in"] == 0.6


def test_subprocess_failure_and_timeout_are_recorded(tmp_path):
    run = benchmark.measure_process
    failed = run(
        [sys.executable, "-c", "raise SystemExit(7)"],
        tmp_path / "failure.log",
        timeout=3,
    )
    assert failed["returncode"] == 7
    assert not failed["timed_out"]
    stalled = run(
        [sys.executable, "-c", "import time; time.sleep(10)"],
        tmp_path / "timeout.log",
        timeout=0.1,
    )
    assert stalled["timed_out"]
    assert stalled["returncode"] != 0
    assert stalled["wall_ms"] > 0
    json.dumps(stalled, allow_nan=False)


def test_successful_measurement_handles_missing_metrics(tmp_path, monkeypatch):
    monkeypatch.setitem(sys.modules, "psutil", None)
    measured = benchmark.measure_process(
        [sys.executable, "-c", "pass"], tmp_path / "ok.log", timeout=3
    )
    assert measured["returncode"] == 0
    assert measured["peak_tree_rss_bytes"] is None
    assert measured["wall_ms"] > 0


@pytest.mark.parametrize(
    "name,device,expected",
    [
        ("llvmpipe", "discretegpu", "software"),
        ("NVIDIA RTX", "discretegpu", "discrete_gpu"),
        ("mystery", "other", "unknown"),
        ("D3D12 (NVIDIA RTX)", "other", "discrete_gpu"),
    ],
)
def test_adapter_classification(name, device, expected):
    assert (
        comparison().adapter_class(
            {"adapter_name": name, "device_type": device, "graphics_backend": "gl"}
        )
        == expected
    )


def test_incomplete_and_failed_runs_cannot_be_summarized():
    c = comparison()
    with pytest.raises(ValueError, match="incomplete|failed"):
        c.summarize([{"status": "failed"}], expected=5)


def test_summary_preserves_null_and_reports_variation():
    c = comparison()
    samples = [
        {
            "status": "ok",
            "wall_ms": n,
            "effective_fps": 300000 / n,
            "peak_tree_rss_bytes": None,
            "cpu_percent": None,
        }
        for n in [100, 200, 300, 400, 500]
    ]
    summary = c.summarize(samples, expected=5)
    assert summary["wall_ms"]["median"] == 300
    assert summary["wall_ms"]["min"] == 100
    assert summary["wall_ms"]["max"] == 500
    assert summary["peak_tree_rss_bytes"] is None


def test_changed_prepared_input_is_rejected(tmp_path):
    c = comparison()
    prepared = {}
    for key in c.SOURCES:
        p = tmp_path / f"{key}.mp4"
        p.write_bytes(b"original")
        prepared[key] = {"filename": p.name, "sha256": c.digest(p)}
    (tmp_path / "manifest.json").write_text(
        json.dumps({"schema_version": 1, "prepared": prepared})
    )
    assert c.load_inputs(tmp_path)["prepared"] == prepared
    (tmp_path / "forest.mp4").write_bytes(b"changed")
    with pytest.raises(ValueError, match="changed or missing"):
        c.load_inputs(tmp_path)


def test_output_rejects_wrong_frame_count(tmp_path, monkeypatch):
    c = comparison()
    monkeypatch.setattr(
        c,
        "probe",
        lambda p: {
            "streams": [
                {
                    "codec_type": "video",
                    "width": 1920,
                    "height": 1080,
                    "avg_frame_rate": "30/1",
                    "nb_read_frames": "299",
                }
            ]
        },
    )
    with pytest.raises(ValueError, match="frame count"):
        c.validate_output(tmp_path / "bad.mp4", c.workload("basic"))


def test_failed_render_persists_failed_record(tmp_path, monkeypatch):
    import argparse

    c = comparison()
    monkeypatch.setattr(c, "load_inputs", lambda d: {})
    monkeypatch.setattr(c, "environment", lambda: {})
    monkeypatch.setattr(
        c,
        "measure_process",
        lambda *a, **kw: {"returncode": 9, "timed_out": False, "wall_ms": 1},
    )
    args = argparse.Namespace(
        inputs=tmp_path,
        output=tmp_path / "run",
        samples=1,
        warmups=0,
        width=1920,
        height=1080,
        timeout=3,
        scenarios=["basic"],
        engines=["vestra-cpu"],
    )
    with pytest.raises(ValueError, match="failed"):
        c.run(args)
    record = json.loads((args.output / "comparison.json").read_text())
    assert record["status"] == "failed"
    assert record["records"][0]["samples"][0]["status"] == "failed"
    assert record["records"][0]["summary"] is None


def test_report_rejects_failed_run(tmp_path):
    c = comparison()
    (tmp_path / "comparison.json").write_text(json.dumps({"status": "failed"}))
    with pytest.raises(ValueError, match="failed|incomplete"):
        c.report([tmp_path])


def test_missing_cpu_metric_is_null(tmp_path, monkeypatch):
    monkeypatch.setitem(sys.modules, "resource", None)
    measured = benchmark.measure_process(
        [sys.executable, "-c", "pass"], tmp_path / "ok.log", timeout=3
    )
    assert measured["cpu_seconds"] is None
    assert measured["cpu_percent"] is None


@pytest.mark.skipif(
    sys.platform != "linux", reason="checks Linux process state after group timeout"
)
def test_timeout_terminates_worker_children(tmp_path):
    pid_file = tmp_path / "child.pid"
    code = (
        "import subprocess,sys,time,pathlib; "
        'child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(10)"]); '
        f"pathlib.Path({str(pid_file)!r}).write_text(str(child.pid)); time.sleep(10)"
    )
    measured = benchmark.measure_process(
        [sys.executable, "-c", code], tmp_path / "timeout.log", timeout=0.5
    )
    assert measured["timed_out"]
    stat = Path("/proc") / pid_file.read_text() / "stat"
    try:
        state = stat.read_text().split(") ", 1)[1]
    except (FileNotFoundError, ProcessLookupError):
        return  # The killed child was already reaped.
    assert state.startswith("Z")


def test_summary_rejects_nonfinite_measurement():
    c = comparison()
    with pytest.raises(ValueError, match="measurement"):
        c.summarize(
            [
                {
                    "status": "ok",
                    "wall_ms": float("nan"),
                    "effective_fps": 1,
                    "peak_tree_rss_bytes": None,
                    "cpu_percent": None,
                }
            ],
            expected=1,
        )
