"""Explicit long-duration public-API renders; never part of the default profile."""

import json
from pathlib import Path
import shutil
import time

from benchmarks.comparisons.moviepy.comparison import validate_output, write_json
from benchmarks.comparisons.moviepy.workloads import (
    render_moviepy,
    render_vestra,
    workload,
)
from benchmarks.collector.fixtures import frame_at
from scripts.benchmark import ROOT, require_backend


def definition(duration, width=640, height=360):
    if duration <= 0 or duration > 7200 or duration * 30 != int(duration * 30):
        raise ValueError(
            "duration must be positive, at most 7200s, and a whole number of frames"
        )
    result = workload("layered", width, height)
    result.update(name="sustained-v1", duration=duration)
    # Two moving placements with a transform, opacity, and animated position.
    # Both read the same long source; no static-frame or encoded-video shortcut.
    result["clips"] = result["clips"][:2]
    for clip in result["clips"]:
        clip.update(asset="forest", start=0, duration=duration)
    result["clips"][1]["brightness"] = 0.03
    return result


def disk_requirement(duration, width, height, engines):
    # Conservative raw yuv420p-sized reservation. CRF sizes are unknowable in
    # advance; runtime free-space checks also guard the 1 GiB reserve.
    return int(duration * 30 * width * height * 1.5 * engines) + 2**30


def worker(request, output):
    definition = request["definition"]
    engine = request["engine"]
    progress_path = Path(request["progress"])
    last = 0.0
    current_frame = 0
    current_total = int(definition["duration"] * definition["fps"])

    def progress(frame, total, **values):
        nonlocal last, current_frame, current_total
        if frame is not None:
            current_frame = frame
        if total is not None:
            current_total = total
        now = time.monotonic()
        if now - last < 1 and current_frame != current_total:
            return
        last = now
        temporary = progress_path.with_suffix(".tmp")
        write_json(
            temporary,
            dict(frame=current_frame, total_frames=current_total, **values),
        )
        try:
            temporary.replace(progress_path)
        except PermissionError:
            temporary.unlink(missing_ok=True)

    inputs = Path(request["inputs"])
    if engine == "moviepy-cpu":
        from proglog import ProgressBarLogger

        class Logger(ProgressBarLogger):
            def bars_callback(self, bar, attr, value, old_value=None):
                if bar == "frame_index" and attr == "index":
                    progress(int(value), int(definition["duration"] * 30))

        result = render_moviepy(
            definition, inputs, Path(request["media"]), progress=Logger()
        )
    else:

        def callback(event):
            progress(
                event.frame, event.total_frames, stage=event.stage, kind=event.kind
            )

        result = render_vestra(
            definition,
            inputs,
            Path(request["media"]),
            "hardware-wgpu" if engine == "hardware-wgpu" else "cpu",
            progress=callback,
        )
    write_json(output, result)


def run(
    directory,
    inputs,
    execute,
    python,
    engines,
    *,
    duration,
    smoke=False,
    timeout=14400,
    retain=False,
):
    directory.mkdir(parents=True)
    size = (128, 72) if smoke else (640, 360)
    work = definition(duration, *size)
    needed = disk_requirement(duration, *size, len(engines) if retain else 1)
    if shutil.disk_usage(directory).free < needed:
        raise ValueError(
            f"sustained rendering needs {needed / 2**30:.1f} GiB free with the conservative output reservation"
        )
    print(
        f"Sustained: {duration}s / {int(duration * 30)} frames per engine, serial execution; disk reservation {needed / 2**30:.1f} GiB. Ctrl+C cancels and preserves diagnostics.",
        flush=True,
    )
    statuses = []
    for engine in engines:
        folder = directory / engine
        folder.mkdir()
        media_dir = folder / "local"
        media_dir.mkdir()
        output = media_dir / "render.mp4"
        record = dict(
            status="incomplete",
            engine=engine,
            duration=duration,
            frames=int(duration * 30),
            definition=work,
            validation=None,
        )
        request = dict(
            engine=engine,
            definition=work,
            inputs=str(inputs.resolve()),
            media=str(output.resolve()),
            progress=str((folder / "progress.json").resolve()),
        )
        request_path = media_dir / "request.json"
        write_json(request_path, request)
        try:
            record["measurement"] = execute(
                [
                    python,
                    str(ROOT / "scripts/collect-benchmarks.py"),
                    "--worker",
                    "sustained",
                    "--request",
                    str(request_path),
                    "--result",
                    str(folder / "render-result.json"),
                ],
                folder / "worker.log",
                timeout=timeout,
                interval=0.1 if smoke else 5,
                series_path=folder / "resources.jsonl",
                progress_path=folder / "progress.json",
                media_directory=media_dir,
                minimum_free_bytes=2**30,
            )
            raw = json.loads((folder / "render-result.json").read_text())
            require_backend(
                raw, "hardware-wgpu" if engine == "hardware-wgpu" else "cpu"
            )
            if raw.get("fallback"):
                raise ValueError("sustained backend fallback is ineligible")
            record["render_result"] = raw
            record["validation"] = validate_output(output, work)
            record["coverage"] = []
            import hashlib

            for frame in [0, int(duration * 30) // 2, int(duration * 30) - 1]:
                pixels = frame_at(output, frame / 30, *size)
                record["coverage"].append(
                    dict(frame=frame, rgb_sha256=hashlib.sha256(pixels).hexdigest())
                )
            record["status"] = "passed"
        except Exception as error:
            record.update(status="failed", error=str(error))
        finally:
            write_json(folder / "result.json", record)
            # Only this invocation's private media directory is eligible.
            if not retain or record["status"] != "passed":
                for path in media_dir.iterdir():
                    if path.suffix == ".mp4":
                        path.unlink(missing_ok=True)
        statuses.append(record)
    if any(r["status"] != "passed" for r in statuses):
        raise ValueError(
            "one or more sustained engines failed; successful evidence preserved"
        )
    return statuses
