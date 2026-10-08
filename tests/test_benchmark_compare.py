"""Comparison tests with hand-calculated timing and resource deltas."""

import copy
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

import pytest

SPEC = importlib.util.spec_from_file_location(
    "benchmark", Path(__file__).resolve().parents[1] / "scripts/benchmark.py"
)
benchmark = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(benchmark)


def record() -> dict:
    """Return a small complete comparison record with two samples."""
    return {
        "benchmark_schema_version": 1,
        "scenario": "masks",
        "workload_id": "fixture-sha256",
        "warmups": 1,
        "environment": {"revision": "a" * 40, "dirty": False, "cpu": "test"},
        "samples": [
            {
                "wall_ms": wall,
                "result": {
                    "width": 1280,
                    "height": 720,
                    "frame_rate": "30/1",
                    "total_frames": 30,
                    "timing_scope": "one_shot",
                    "requested_render_backend": "cpu",
                    "render_backend": "cpu",
                    "encoder_backend": "ffmpeg",
                    "timings": {
                        "frame_render_ms": wall - 10,
                        "gpu_submission_ms": None,
                    },
                    "performance": {
                        "peak_decoded_bytes": memory,
                        "static_visual_ffmpeg_fast_path_used": False,
                        "encoder_video_input_mode": "rgba",
                    },
                },
            }
            for wall, memory in [(100, 1000), (200, 2000)]
        ],
    }


def test_compare_uses_all_samples_and_reports_resource_deltas() -> None:
    before = record()
    after = copy.deepcopy(before)
    after["environment"]["revision"] = "b" * 40
    after["samples"][0]["wall_ms"] = 80
    after["samples"][1]["wall_ms"] = 160
    after["samples"][0]["result"]["performance"]["peak_decoded_bytes"] = 2000
    rows = {row["metric"]: row for row in benchmark.compare(before, after)}
    assert rows["wall_ms"] == {
        "metric": "wall_ms",
        "before": 150,
        "after": 120,
        "delta": -30,
        "percent": -20,
    }
    assert rows["performance.peak_decoded_bytes"]["delta"] == 500
    assert "timings.gpu_submission_ms" not in rows


@pytest.mark.parametrize("field", ["scenario", "workload_id", "warmups"])
def test_compare_rejects_changed_workload(field: str) -> None:
    before = record()
    after = copy.deepcopy(before)
    after[field] = "different"
    with pytest.raises(ValueError, match="incompatible"):
        benchmark.compare(before, after)


def test_compare_rejects_backend_change_in_any_sample() -> None:
    before = record()
    after = copy.deepcopy(before)
    after["samples"][1]["result"]["render_backend"] = "wgpu"
    with pytest.raises(ValueError, match="incompatible"):
        benchmark.compare(before, after)


def test_compare_rejects_environment_changes() -> None:
    before = record()
    after = copy.deepcopy(before)
    after["environment"]["cpu"] = "another CPU"
    with pytest.raises(ValueError, match="incompatible"):
        benchmark.compare(before, after)


def test_compare_rejects_missing_samples() -> None:
    before = record()
    after = copy.deepcopy(before)
    after["samples"] = []
    with pytest.raises(ValueError, match="samples"):
        benchmark.compare(before, after)


def test_zero_baseline_has_no_percentage() -> None:
    before = record()
    after = copy.deepcopy(before)
    for sample in before["samples"]:
        sample["wall_ms"] = 0
    rows = {row["metric"]: row for row in benchmark.compare(before, after)}
    assert rows["wall_ms"]["percent"] is None


def test_compare_cli_outputs_json_deltas(tmp_path: Path) -> None:
    """Users can compare saved reports without importing Python helpers."""
    before = tmp_path / "before.json"
    after = tmp_path / "after.json"
    before.write_text(json.dumps(record()))
    after.write_text(json.dumps(record()))
    result = subprocess.run(
        [
            sys.executable,
            str(Path(benchmark.__file__)),
            "compare",
            str(before),
            str(after),
            "--json",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    rows = json.loads(result.stdout)
    assert rows[0]["metric"] == "wall_ms"
    assert rows[0]["delta"] == 0


@pytest.mark.parametrize(
    "device,accepted",
    [
        ("discretegpu", True),
        ("integratedgpu", True),
        ("cpu", False),
        ("virtualgpu", False),
    ],
)
def test_hardware_suite_requires_confirmed_hardware(
    device: str, accepted: bool
) -> None:
    result = {"render_backend": "wgpu", "adapter": {"device_type": device}}
    if accepted:
        benchmark.require_backend(result, "hardware-wgpu")
    else:
        with pytest.raises(ValueError, match="hardware-GPU blocked"):
            benchmark.require_backend(result, "hardware-wgpu")


@pytest.mark.parametrize(
    "mutation",
    ["missing_workload", "duplicate_workload", "missing_sample", "wrong_size"],
)
def test_suite_comparison_rejects_incomplete_or_mislabeled_results(
    mutation: str,
) -> None:
    before = {
        "suite_schema_version": 1,
        "definition": {
            "scenarios": ["masks"],
            "settings": {"width": 1280, "height": 720, "samples": 2, "warmups": 1},
        },
        "records": [record()],
    }
    after = copy.deepcopy(before)
    if mutation == "missing_workload":
        after["records"] = []
    elif mutation == "duplicate_workload":
        after["records"].append(record())
    elif mutation == "missing_sample":
        after["records"][0]["samples"].pop()
    else:
        for document in (before, after):
            for sample in document["records"][0]["samples"]:
                sample["result"]["width"] = 64
    with pytest.raises(ValueError, match="incomplete|incompatible"):
        benchmark.compare_documents(before, after)


def test_stylization_suite_selects_only_three_fullhd_workloads():
    manifest = json.loads(
        (Path(benchmark.__file__).parents[1] / "benchmarks/suites.json").read_text()
    )
    assert hasattr(benchmark, "suite_configuration")
    settings, scenarios = benchmark.suite_configuration(manifest, "stylization-1080p")
    assert settings == {"width": 1920, "height": 1080, "warmups": 1, "samples": 3}
    assert scenarios == ["stylization_baseline", "palette_video", "dither_video"]
    assert benchmark.suite_configuration(manifest, "canonical") == (
        {"width": 1280, "height": 720, "warmups": 1, "samples": 5},
        [
            "single_video",
            "mixed_dynamic",
            "video_heavy",
            "combined",
            "masks",
            "mattes",
            "nested_groups",
            "particles",
            "blend_modes",
            "production_edit",
        ],
    )


def test_stylization_smoke_selects_same_workloads_at_small_resolution():
    manifest = json.loads(
        (Path(benchmark.__file__).parents[1] / "benchmarks/suites.json").read_text()
    )
    assert hasattr(benchmark, "suite_configuration")
    settings, scenarios = benchmark.suite_configuration(manifest, "stylization-smoke")
    assert settings == {"width": 128, "height": 72, "warmups": 0, "samples": 1}
    assert scenarios == ["stylization_baseline", "palette_video", "dither_video"]


def test_suite_override_does_not_mutate_manifest_or_export_scenarios_as_environment():
    manifest = {
        "scenarios": ["canonical"],
        "suites": {
            "focused": {
                "width": 64,
                "height": 64,
                "samples": 1,
                "warmups": 0,
                "scenarios": ["palette_video"],
            }
        },
    }
    original = copy.deepcopy(manifest)
    assert hasattr(benchmark, "suite_configuration")
    settings, scenarios = benchmark.suite_configuration(manifest, "focused")
    assert "scenarios" not in settings
    assert scenarios == ["palette_video"]
    assert manifest == original
