"""Real release-benchmark contract test, supplied an executable by the caller."""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest


@pytest.mark.skipif(
    "VESTRA_BENCH_EXECUTABLE" not in os.environ, reason="requires release executable"
)
@pytest.mark.parametrize(
    "suite_name,expected_count", [("smoke", 10), ("stylization-smoke", 3)]
)
def test_smoke_suite_records_every_workload(
    tmp_path: Path, suite_name: str, expected_count: int
) -> None:
    """A completed suite records all workloads and compares with itself."""
    script = Path(__file__).resolve().parents[1] / "scripts/benchmark.py"
    output = tmp_path / "run"
    subprocess.run(
        [
            sys.executable,
            str(script),
            "run",
            "--suite",
            suite_name,
            "--executable",
            os.environ["VESTRA_BENCH_EXECUTABLE"],
            "--output",
            str(output),
        ],
        check=True,
    )
    suite = json.loads((output / "suite.json").read_text())
    assert len(suite["provenance"]["executable_sha256"]) == 64
    assert len(suite["provenance"]["source_sha256"]) == 64
    assert len(suite["records"]) == expected_count
    expected = (
        {"masks", "mattes", "particles", "nested_groups"}
        if suite_name == "smoke"
        else {"stylization_baseline", "palette_video", "dither_video"}
    )
    assert {item["scenario"] for item in suite["records"]} >= expected
    result = subprocess.run(
        [
            sys.executable,
            str(script),
            "compare",
            str(output / "suite.json"),
            str(output / "suite.json"),
            "--json",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    assert all(row["delta"] == 0 for row in json.loads(result.stdout))


@pytest.mark.skipif(
    "VESTRA_BENCH_EXECUTABLE" not in os.environ,
    reason="requires the release benchmark executable",
)
def test_benchmark_records_all_samples(tmp_path: Path) -> None:
    """Missing samples or metadata must fail independently of console output."""
    report = tmp_path / "result.json"
    environment = {
        **os.environ,
        "VESTRA_BENCH_BACKEND": "cpu",
        "VESTRA_BENCH_SCENARIO": "short_sequence",
        "VESTRA_BENCH_WIDTH": "64",
        "VESTRA_BENCH_HEIGHT": "64",
        "VESTRA_BENCH_WARMUPS": "0",
        "VESTRA_BENCH_SAMPLES": "2",
        "VESTRA_BENCH_REPORT": str(report),
    }
    subprocess.run([os.environ["VESTRA_BENCH_EXECUTABLE"]], env=environment, check=True)
    assert report.is_file(), "benchmark must write structured results"
    record = json.loads(report.read_text())
    assert record["benchmark_schema_version"] == 1
    assert record["scenario"] == "short_sequence"
    assert record["warmups"] == 0
    assert len(record["samples"]) == 2
    assert len(record["environment"]["revision"]) == 40
    assert record["environment"]["ffmpeg"].startswith("ffmpeg version")
    for sample in record["samples"]:
        assert sample["wall_ms"] > 0
        assert sample["result"]["total_frames"] == 24
        assert sample["result"]["render_backend"] == "cpu"
        assert sample["result"]["timing_scope"] == "one_shot"
        assert "frame_render_ms" in sample["result"]["timings"]
        assert "peak_decoded_bytes" in sample["result"]["performance"]


@pytest.mark.skipif(
    "VESTRA_BENCH_EXECUTABLE" not in os.environ, reason="requires release executable"
)
@pytest.mark.parametrize(
    "scenario,frames",
    [
        ("masks", 30),
        ("mattes", 30),
        ("nested_groups", 90),
        ("particles", 30),
        ("production_edit", 360),
        ("video_heavy", 90),
    ],
)
def test_modern_workloads_render(tmp_path: Path, scenario: str, frames: int) -> None:
    """Each registered modern workload must validate and render all its frames."""
    report = tmp_path / "result.json"
    subprocess.run(
        [os.environ["VESTRA_BENCH_EXECUTABLE"]],
        check=True,
        env={
            **os.environ,
            "VESTRA_BENCH_BACKEND": "cpu",
            "VESTRA_BENCH_SCENARIO": scenario,
            "VESTRA_BENCH_WIDTH": "128",
            "VESTRA_BENCH_HEIGHT": "72",
            "VESTRA_BENCH_WARMUPS": "0",
            "VESTRA_BENCH_SAMPLES": "1",
            "VESTRA_BENCH_REPORT": str(report),
        },
    )
    result = json.loads(report.read_text())["samples"][0]["result"]
    assert result["total_frames"] == frames
    assert result["render_backend"] == "cpu"
    if scenario in {"production_edit", "video_heavy"}:
        assert result["performance"]["video_actual_decodes"] > 0
    if scenario == "production_edit":
        assert result["audio_present"] is True


@pytest.mark.skipif(
    "VESTRA_BENCH_EXECUTABLE" not in os.environ, reason="requires release executable"
)
def test_generated_video_identity_survives_fresh_directories(tmp_path: Path) -> None:
    """Regenerating identical media must not make before/after incompatible."""
    identities = []
    for index in range(2):
        report = tmp_path / f"{index}.json"
        subprocess.run(
            [os.environ["VESTRA_BENCH_EXECUTABLE"]],
            check=True,
            env={
                **os.environ,
                "VESTRA_BENCH_BACKEND": "cpu",
                "VESTRA_BENCH_SCENARIO": "single_video",
                "VESTRA_BENCH_WIDTH": "128",
                "VESTRA_BENCH_HEIGHT": "72",
                "VESTRA_BENCH_WARMUPS": "0",
                "VESTRA_BENCH_SAMPLES": "1",
                "VESTRA_BENCH_REPORT": str(report),
            },
        )
        identities.append(json.loads(report.read_text())["workload_id"])
    assert identities[0] == identities[1]


@pytest.mark.skipif(
    "VESTRA_BENCH_EXECUTABLE" not in os.environ, reason="requires release executable"
)
def test_stylization_workloads_share_only_the_same_moving_video(tmp_path: Path) -> None:
    records = []
    for scenario in ("stylization_baseline", "palette_video", "dither_video"):
        report = tmp_path / f"{scenario}.json"
        subprocess.run(
            [os.environ["VESTRA_BENCH_EXECUTABLE"]],
            check=True,
            env={
                **os.environ,
                "VESTRA_BENCH_BACKEND": "cpu",
                "VESTRA_BENCH_SCENARIO": scenario,
                "VESTRA_BENCH_WIDTH": "128",
                "VESTRA_BENCH_HEIGHT": "72",
                "VESTRA_BENCH_WARMUPS": "0",
                "VESTRA_BENCH_SAMPLES": "1",
                "VESTRA_BENCH_REPORT": str(report),
            },
        )
        record = json.loads(report.read_text())
        workload = record["workload"]
        assert len(workload["assets"]) == 1
        assert workload["assets"][0]["type"] == "video"
        clips = workload["visual"]["clips"]
        assert len(clips) == 1 and clips[0]["source"]["type"] == "video"
        assert clips[0]["duration"] == 3 and clips[0]["playback_rate"] == 1
        assert not workload["visual"]["post_effects"]
        result = record["samples"][0]["result"]
        assert result["total_frames"] == 90
        assert result["frame_rate"] == "30/1"
        assert result["performance"]["video_actual_decodes"] > 1
        records.append(record)
    assert len({record["workload"]["assets"][0]["source"] for record in records}) == 1
    assert not records[0]["workload"]["visual"]["clips"][0]["effects"]
    assert (
        records[1]["workload"]["visual"]["clips"][0]["effects"][0]["type"]
        == "palette_map"
    )
    assert (
        records[2]["workload"]["visual"]["clips"][0]["effects"][0]["type"]
        == "ordered_dither"
    )
