"""Decoded-frame parity and independent source checks, outside measured runs."""

import importlib.util
import json
import subprocess
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageStat

from benchmarks.comparisons.moviepy.comparison import digest, load_inputs, write_json
from benchmarks.comparisons.moviepy.workloads import workload
from scripts.benchmark import ROOT, require_backend

FRAMES = {
    "basic": [0, 1, 150, 299],
    "editing": [0, 113, 114, 123, 131, 132, 150, 227, 228, 237, 245, 246, 300, 359],
    "layered": [0, 29, 30, 89, 90, 180, 269, 270, 299],
}
# Quarter-size RGB smooths bilinear/Lanczos detail differences; retain full-size raw errors.
MAX_RGB_MAE = 8


def extract_frames(video: Path, frames: list[int], output: Path) -> dict[int, Path]:
    """Decode once in presentation order, selecting exact integer frame indices."""
    output.mkdir(parents=True, exist_ok=False)
    frames = sorted(set(frames))
    selection = "+".join(f"eq(n\\,{frame})" for frame in frames)
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-xerror",
            "-nostdin",
            "-threads",
            "1",
            "-i",
            str(video),
            "-map",
            "0:v:0",
            "-vf",
            f"select={selection},format=rgb24",
            "-fps_mode",
            "passthrough",
            "-frames:v",
            str(len(frames)),
            "-threads",
            "1",
            str(output / "frame-%04d.png"),
        ],
        check=True,
        capture_output=True,
    )
    paths = {
        frame: output / f"frame-{index:04d}.png"
        for index, frame in enumerate(frames, 1)
    }
    if any(not path.is_file() for path in paths.values()):
        raise ValueError(f"{video.name}: missing requested frame(s) {frames}")
    return paths


def visual_tool(name):
    path = ROOT / ".agents/skills/visual-regression/scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def eligible(runs, manifest):
    scenarios, identity = {}, None
    for directory in runs:
        record = json.loads((directory / "comparison.json").read_text())
        if record.get("status") != "ok" or record.get("comparison_schema_version") != 1:
            raise ValueError("failed or incomplete comparison run")
        environment = {
            k: v
            for k, v in record["environment"].items()
            if k not in ("recorded_at", "revision", "dirty")
        }
        current = (environment, record["input_manifest"])
        if (
            not environment.get("source_sha256")
            or current[1] != manifest
            or (identity is not None and current != identity)
        ):
            raise ValueError(
                "incompatible environment, source or prepared input identity"
            )
        identity = current
        if not record["records"]:
            raise ValueError("incomplete comparison records")
        for row in record["records"]:
            name, engine, definition = row["scenario"], row["engine"], row["definition"]
            if definition != workload(name, definition["width"], definition["height"]):
                raise ValueError(f"{name}: incompatible workload definition")
            entries = scenarios.setdefault(name, {})
            if engine in entries:
                raise ValueError(f"{name}: duplicate engine {engine}")
            if engine not in ("vestra-cpu", "moviepy-cpu", "hardware-wgpu"):
                raise ValueError(f"unknown engine: {engine}")
            if entries and definition != next(iter(entries.values()))[1]:
                raise ValueError(f"{name}: incompatible workload definition")
            for field in ("samples", "warmups"):
                samples = row[field]
                if len(samples) != record["settings"][field] or (
                    field == "samples" and not samples
                ):
                    raise ValueError("incomplete comparison samples")
                if any(
                    s.get("status") != "ok"
                    or s.get("returncode") != 0
                    or s.get("timed_out")
                    or s.get("validation", {}).get("status") != "ok"
                    for s in samples
                ):
                    raise ValueError("failed or incomplete comparison sample")
                for sample in samples:
                    require_backend(
                        sample["result"],
                        "hardware-wgpu" if engine == "hardware-wgpu" else "cpu",
                    )
                    if sample["result"].get("fallback"):
                        raise ValueError(
                            "backend fallback is ineligible for comparison"
                        )
            filename = Path(row["samples"][0]["output"])
            if filename.name != str(filename):
                raise ValueError("output must be a filename within its run directory")
            stored_hash = row["samples"][0]["validation"].get("sha256")
            if stored_hash and digest(directory / filename) != stored_hash:
                raise ValueError(f"published output changed: {filename}")
            entries[engine] = (directory / filename, definition)
    if not scenarios or any(len(entries) < 2 for entries in scenarios.values()):
        raise ValueError("each scenario requires at least two distinct engines")
    return scenarios


def active_clips(definition, frame):
    return [
        c
        for c in definition["clips"]
        if round(c["start"] * 30) <= frame < round((c["start"] + c["duration"]) * 30)
    ]


def source_reference(sources, definition, frame, output):
    """Composite prepared source frames with Pillow, independently of either engine."""
    w, h = definition["width"], definition["height"]
    canvas = Image.new("RGB", (w, h))
    for clip in active_clips(definition, frame):
        local_frame = frame - round(clip["start"] * 30)
        with Image.open(sources[clip["asset"]][local_frame]) as image:
            image = image.convert("RGB").resize((w, h), Image.Resampling.BICUBIC)
        image = image.point(
            lambda v: min(255, max(0, int(v + 255 * clip["brightness"])))
        )
        mask = Image.new("L", (w, h), 0 if clip["ellipse"] else 255)
        if clip["ellipse"]:
            ImageDraw.Draw(mask).ellipse(
                (w * 0.05, h * 0.05, w * 0.95, h * 0.95), fill=255
            )
        size = (int(w * clip["scale"]), int(h * clip["scale"]))
        image, mask = (
            im.resize(size, Image.Resampling.BICUBIC) for im in (image, mask)
        )
        local = local_frame / 30
        opacity = clip["opacity"] * (
            min(1, local / clip["fade_in"]) if clip["fade_in"] else 1
        )
        mask = mask.point(lambda v: int(v * opacity))
        x = clip["x"] + (clip["end_x"] - clip["x"]) * local / clip["duration"]
        canvas.paste(
            image,
            (int(x * w - image.width / 2), int(clip["y"] * h - image.height / 2)),
            mask,
        )
    canvas.save(output)
    return output


def verify(
    runs: list[Path], output: Path, inputs: Path = Path("target/moviepy-inputs")
) -> dict:
    """Verify the first measured output of each engine; write evidence, then fail if needed."""
    manifest = load_inputs(inputs)
    scenarios = eligible(runs, manifest)
    output.mkdir(parents=True, exist_ok=False)
    compare = visual_tool("compare_frames").compare
    create_sheet = visual_tool("create_contact_sheet").create_sheet
    report = dict(
        status="running",
        selection="first measured sample per engine/scenario",
        captures=[digest(p / "comparison.json") for p in runs],
        outputs=[
            dict(scenario=n, engine=e, filename=p.name, sha256=digest(p))
            for n, entries in scenarios.items()
            for e, (p, _) in entries.items()
        ],
        threshold=dict(
            max_rgb_mae=MAX_RGB_MAE, reduction_divisor=4, resized_maximum=[480, 270]
        ),
        frames=[],
    )

    def measure(reference, candidate, directory, definition, **metadata):
        raw = compare(reference, candidate, directory, tolerance=8)
        with Image.open(reference) as a, Image.open(candidate) as b:
            areas = [("full", (0, 0, a.width, a.height))]
            for clip in active_clips(definition, metadata["frame"])[1:]:
                t = (metadata["frame"] / 30 - clip["start"]) / clip["duration"]
                x, y = clip["x"] + (clip["end_x"] - clip["x"]) * t, clip["y"]
                for radius in (0.3, 0.5) if clip["ellipse"] else (0.3,):
                    dx = radius * clip["scale"]
                    box = tuple(
                        map(
                            int,
                            (
                                (x - dx) * a.width,
                                (y - dx) * a.height,
                                (x + dx) * a.width,
                                (y + dx) * a.height,
                            ),
                        )
                    )
                    label = clip["asset"] + ("-mask-bounds" if radius == 0.5 else "")
                    areas.append((label, box))
            errors = []
            for label, box in areas:
                width, height = max(1, box[2] - box[0]), max(1, box[3] - box[1])
                size = (min(480, max(1, width // 4)), min(270, max(1, height // 4)))
                pair = [
                    im.crop(box).convert("RGB").resize(size, Image.Resampling.LANCZOS)
                    for im in (a, b)
                ]
                mae = sum(ImageStat.Stat(ImageChops.difference(*pair)).mean) / 3
                errors.append(dict(region=label, box=box, rgb_mae=mae))
        result = dict(
            **metadata,
            raw=raw,
            rgb_mae=errors[0]["rgb_mae"],
            regions=errors[1:],
            passed=all(e["rgb_mae"] < MAX_RGB_MAE for e in errors),
        )
        report["frames"].append(result)

    requests = {}
    for name, entries in scenarios.items():
        definition = next(iter(entries.values()))[1]
        for frame in FRAMES[name]:
            for clip in active_clips(definition, frame):
                local_frame = frame - round(clip["start"] * 30)
                requests.setdefault(clip["asset"], set()).add(local_frame)
    sources = {
        asset: extract_frames(
            inputs / manifest["prepared"][asset]["filename"],
            sorted(frames),
            output / "sources" / asset,
        )
        for asset, frames in requests.items()
    }
    for name, entries in scenarios.items():
        definition = next(iter(entries.values()))[1]
        frames = {
            engine: extract_frames(video, FRAMES[name], output / name / engine)
            for engine, (video, _) in entries.items()
        }
        reference_engine = (
            "moviepy-cpu" if "moviepy-cpu" in entries else next(iter(entries))
        )
        images, labels = [], []
        for frame in FRAMES[name]:
            for engine, paths in frames.items():
                images.append(paths[frame])
                labels.append(f"{name} frame {frame} {engine}")
                if engine != reference_engine:
                    measure(
                        frames[reference_engine][frame],
                        paths[frame],
                        output / name / f"parity-{engine}-{frame}",
                        definition,
                        kind="engine-parity",
                        scenario=name,
                        frame=frame,
                        engine=engine,
                        reference_engine=reference_engine,
                    )
            expected = source_reference(
                sources, definition, frame, output / name / f"expected-{frame}.png"
            )
            for engine, paths in frames.items():
                measure(
                    expected,
                    paths[frame],
                    output / name / f"source-{engine}-{frame}",
                    definition,
                    kind="source-reference",
                    scenario=name,
                    frame=frame,
                    engine=engine,
                )
        create_sheet(
            images,
            labels,
            output / name / "contact.png",
            len(entries),
            definition["width"],
            definition["height"],
        )
    report["status"] = "ok" if all(m["passed"] for m in report["frames"]) else "failed"
    write_json(output / "metrics.json", report)
    if report["status"] != "ok":
        raise ValueError(
            f"visual equivalence failed; inspect {output / 'metrics.json'}"
        )
    return report
