"""Real decoded-frame checks for the external comparison; no native build."""

import copy
import importlib.util
import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from benchmarks.comparisons.moviepy import comparison  # noqa: E402


def validator():
    path = ROOT / "benchmarks/comparisons/moviepy/validation.py"
    assert path.is_file(), "visual verifier is missing"
    spec = importlib.util.spec_from_file_location("moviepy_validation", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def runs(tmp_path):
    if not shutil.which("ffmpeg"):
        pytest.skip("FFmpeg is required for decoded-frame verification")
    inputs = tmp_path / "inputs"
    inputs.mkdir()
    video = inputs / "forest.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=32x18:rate=30:duration=10",
            "-an",
            "-c:v",
            "libx264",
            "-crf",
            "0",
            "-threads",
            "1",
            str(video),
        ],
        check=True,
    )
    prepared = {}
    for asset in ("forest", "valley", "tear"):
        path = inputs / f"{asset}.mp4"
        if path != video:
            shutil.copyfile(video, path)
        prepared[asset] = dict(filename=path.name, sha256=comparison.digest(path))
    manifest = dict(
        schema_version=1, sources={"forest": {"sha256": "original"}}, prepared=prepared
    )
    comparison.write_json(inputs / "manifest.json", manifest)
    record = dict(
        comparison_schema_version=1,
        status="ok",
        environment={"source_sha256": "same"},
        input_manifest=manifest,
        settings={"samples": 1, "warmups": 0},
        records=[],
    )
    directory = tmp_path / "run"
    directory.mkdir()
    for engine in ("vestra-cpu", "moviepy-cpu"):
        name = f"basic-{engine}.mp4"
        shutil.copyfile(video, directory / name)
        record["records"].append(
            dict(
                scenario="basic",
                engine=engine,
                definition=comparison.workload("basic", 32, 18),
                warmups=[],
                samples=[
                    dict(
                        status="ok",
                        output=name,
                        returncode=0,
                        timed_out=False,
                        result={"render_backend": "cpu", "fallback": None},
                        validation={"status": "ok"},
                    )
                ],
            )
        )
    comparison.write_json(directory / "comparison.json", record)
    return directory, inputs, record


def test_matching_outputs_have_per_frame_metrics_and_contact_sheet(runs, tmp_path):
    run, inputs, _ = runs
    report = validator().verify([run], tmp_path / "evidence", inputs)
    assert report["status"] == "ok"
    parity = [m for m in report["frames"] if m["kind"] == "engine-parity"]
    assert [m["frame"] for m in parity] == [0, 1, 150, 299]
    assert all(m["raw"]["mae"] == 0 for m in parity)
    assert report["captures"] == [comparison.digest(run / "comparison.json")]
    assert {m["sha256"] for m in report["outputs"]} == {
        comparison.digest(inputs / "forest.mp4")
    }
    assert (tmp_path / "evidence/basic/contact.png").is_file()
    assert json.loads((tmp_path / "evidence/metrics.json").read_text()) == report


def test_wrong_output_pixels_fail_with_saved_evidence(runs, tmp_path):
    run, inputs, record = runs
    output = run / record["records"][0]["samples"][0]["output"]
    output.unlink()
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=black:size=32x18:rate=30:duration=10",
            "-an",
            "-c:v",
            "libx264",
            "-threads",
            "1",
            str(output),
        ],
        check=True,
    )
    with pytest.raises(ValueError, match="visual equivalence"):
        validator().verify([run], tmp_path / "evidence", inputs)
    report = json.loads((tmp_path / "evidence/metrics.json").read_text())
    assert report["status"] == "failed"
    assert any(not frame["passed"] for frame in report["frames"])


def test_agreeing_engines_cannot_hide_wrong_source_selection(runs, tmp_path):
    run, inputs, record = runs
    source = inputs / "forest.mp4"
    source.unlink()
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=black:size=32x18:rate=30:duration=10",
            "-an",
            "-c:v",
            "libx264",
            "-threads",
            "1",
            str(source),
        ],
        check=True,
    )
    manifest = record["input_manifest"]
    manifest["prepared"]["forest"]["sha256"] = comparison.digest(source)
    comparison.write_json(inputs / "manifest.json", manifest)
    comparison.write_json(run / "comparison.json", record)
    with pytest.raises(ValueError, match="visual equivalence"):
        validator().verify([run], tmp_path / "evidence", inputs)
    frames = json.loads((tmp_path / "evidence/metrics.json").read_text())["frames"]
    assert all(m["passed"] for m in frames if m["kind"] == "engine-parity")
    assert any(not m["passed"] for m in frames if m["kind"] == "source-reference")


@pytest.mark.parametrize(
    "failure", ["run", "sample", "duplicate", "definition", "identity", "input"]
)
def test_ineligible_records_are_rejected_before_decode(runs, tmp_path, failure):
    run, inputs, record = runs
    directories = [run]
    if failure == "run":
        record["status"] = "failed"
    elif failure == "sample":
        record["records"][0]["samples"][0]["status"] = "failed"
    elif failure == "duplicate":
        record["records"].append(copy.deepcopy(record["records"][0]))
    elif failure == "definition":
        record["records"][0]["definition"]["duration"] = 9
    elif failure == "input":
        record["input_manifest"]["sources"]["forest"]["sha256"] = "changed"
    else:
        second = tmp_path / "other"
        second.mkdir()
        different = copy.deepcopy(record)
        different["environment"]["source_sha256"] = "changed"
        comparison.write_json(second / "comparison.json", different)
        directories.append(second)
    comparison.write_json(run / "comparison.json", record)
    with pytest.raises(
        ValueError, match="failed|incomplete|duplicate|definition|identity"
    ):
        validator().verify(directories, tmp_path / "evidence", inputs)


@pytest.mark.parametrize("failure", ["backend", "hardware", "fallback", "environment"])
def test_claimed_success_cannot_hide_invalid_execution(runs, tmp_path, failure):
    run, inputs, record = runs
    directories = [run]
    row = record["records"][0]
    if failure == "backend":
        row["samples"][0]["result"]["render_backend"] = "wgpu"
    elif failure == "hardware":
        row["engine"] = "hardware-wgpu"
    elif failure == "fallback":
        row["samples"][0]["result"]["fallback"] = "adapter unavailable"
    else:
        second = tmp_path / "other"
        second.mkdir()
        different = copy.deepcopy(record)
        different["environment"]["packages"] = {"moviepy": "different"}
        comparison.write_json(second / "comparison.json", different)
        directories.append(second)
    comparison.write_json(run / "comparison.json", record)
    with pytest.raises(ValueError, match="backend|hardware|fallback|environment"):
        validator().verify(directories, tmp_path / "evidence", inputs)


def test_changed_published_output_is_rejected(runs, tmp_path):
    run, inputs, record = runs
    row = record["records"][0]["samples"][0]
    row["validation"]["sha256"] = comparison.digest(run / row["output"])
    comparison.write_json(run / "comparison.json", record)
    (run / row["output"]).write_bytes(b"changed after measurement")
    with pytest.raises(ValueError, match="output.*changed"):
        validator().verify([run], tmp_path / "evidence", inputs)


@pytest.mark.parametrize("both_wrong", [False, True])
@pytest.mark.parametrize("failure", ["overlay", "mask"])
def test_missing_mask_or_overlay_fails_local_check(runs, tmp_path, both_wrong, failure):
    from PIL import Image, ImageDraw

    run, inputs, record = runs
    size, colors = (
        (96, 54),
        {"forest": (40, 80, 120), "valley": (50, 90, 130), "tear": (180, 180, 180)},
    )

    def encode(path, frames):
        path.unlink(missing_ok=True)
        process = subprocess.Popen(
            [
                "ffmpeg",
                "-v",
                "error",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgb24",
                "-video_size",
                "96x54",
                "-framerate",
                "30",
                "-i",
                "-",
                "-an",
                "-c:v",
                "libx264",
                "-crf",
                "0",
                "-threads",
                "1",
                str(path),
            ],
            stdin=subprocess.PIPE,
        )
        for frame in frames:
            process.stdin.write(frame.tobytes())
        process.stdin.close()
        assert process.wait() == 0

    for asset, color in colors.items():
        path = inputs / f"{asset}.mp4"
        encode(path, [Image.new("RGB", size, color)] * 300)
        record["input_manifest"]["prepared"][asset]["sha256"] = comparison.digest(path)

    def frames(include_tear, ellipse):
        for n in range(300):
            image = Image.new("RGB", size, colors["forest"])
            if 30 <= n < 270:
                x = int((0.30 + 0.08 * (n - 30) / 240) * 96 - 24)
                image.paste(
                    Image.new("RGB", (48, 27), colors["valley"]),
                    (x, 13),
                    Image.new("L", (48, 27), int(255 * 0.85)),
                )
            if include_tear and 90 <= n < 270:
                x = int((0.72 - 0.06 * (n - 90) / 180) * 96 - 18)
                mask = Image.new("L", (36, 20), 0 if ellipse else int(255 * 0.7))
                if ellipse:
                    ImageDraw.Draw(mask).ellipse(
                        (1.8, 1, 34.2, 19), fill=int(255 * 0.7)
                    )
                image.paste(Image.new("RGB", (36, 20), colors["tear"]), (x, 18), mask)
            yield image

    for row in record["records"]:
        row["scenario"], row["definition"] = (
            "layered",
            comparison.workload("layered", *size),
        )
        wrong = row["engine"] != "moviepy-cpu" or both_wrong
        include, ellipse = (
            failure == "mask" or not wrong,
            not (failure == "mask" and wrong),
        )
        encode(run / row["samples"][0]["output"], frames(include, ellipse))
    comparison.write_json(inputs / "manifest.json", record["input_manifest"])
    comparison.write_json(run / "comparison.json", record)
    with pytest.raises(ValueError, match="visual equivalence"):
        validator().verify([run], tmp_path / "evidence", inputs)
    checks = json.loads((tmp_path / "evidence/metrics.json").read_text())["frames"]
    relevant = [m for m in checks if m["scenario"] == "layered" and m["frame"] == 180]
    assert any(m["rgb_mae"] < 8 and not m["passed"] for m in relevant)
    if both_wrong:
        assert all(m["passed"] for m in checks if m["kind"] == "engine-parity")
        assert any(not m["passed"] for m in checks if m["kind"] == "source-reference")


def test_one_pass_extraction_preserves_requested_integer_frames(runs, tmp_path):
    _, inputs, _ = runs
    paths = validator().extract_frames(
        inputs / "forest.mp4", [0, 150, 299], tmp_path / "frames"
    )
    from PIL import Image

    assert set(paths) == {0, 150, 299}
    with Image.open(paths[0]) as first, Image.open(paths[299]) as last:
        assert first.size == last.size == (32, 18)
        assert first.tobytes() != last.tobytes()
    with pytest.raises(ValueError, match="missing.*frame"):
        validator().extract_frames(inputs / "forest.mp4", [300], tmp_path / "missing")
