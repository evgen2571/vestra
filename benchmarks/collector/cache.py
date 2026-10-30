"""Controlled native cache A/B and MoviePy public frame access."""

import hashlib
import json
import os
from pathlib import Path
import random
from statistics import median
import time

from benchmarks.comparisons.moviepy.comparison import write_json


def patterns(duration=7200, *, smoke=False, fps=30, cache_frames=8):
    count = 12 if smoke else 96
    end = int(duration * fps) - 1
    rng = random.Random(2571)
    sequences = {
        "sequential": list(range(count)),
        "repeated_frame": [15] * count,
        "local_reuse": [0, 1, 2, 3, 2, 1, 0, 3] * (2 if smoke else 12),
        "random_seeking": [rng.randrange(end) for _ in range(4 if smoke else 12)],
        "eviction": list(range(cache_frames * 3)) + [0, 1, 2, 0],
        "repeated_placements": [
            i // 2 + (0 if i % 2 == 0 else fps * 6) for i in range(count)
        ],
    }
    return {
        name: [
            dict(
                session=i % 2 if name == "repeated_placements" else 0,
                seconds=(min(frame, end) + 0.25) / fps,
            )
            for i, frame in enumerate(frames)
        ]
        for name, frames in sequences.items()
    }


def moviepy_access(request, output):
    """Use get_frame normally; do not infer FFmpeg decode/cache counters."""
    import shutil

    os.environ["FFMPEG_BINARY"] = shutil.which("ffmpeg") or "ffmpeg"
    from moviepy import VideoFileClip

    clips = [
        VideoFileClip(request["source"], audio=False)
        for _ in range(request["sessions"])
    ]
    latencies, hashes = [], []
    try:
        for item in request["requests"]:
            started = time.perf_counter_ns()
            frame = clips[item["session"]].get_frame(item["seconds"])
            latencies.append((time.perf_counter_ns() - started) / 1000)
            # Outside access timing; only correctness workers scan pixels.
            if request["verify"]:
                pixels = frame.tobytes()
                from benchmarks.collector.fixtures import frame_at

                # A fractional request denotes the covering CFR frame, whereas
                # FFmpeg's output-side seek selects the next timestamp. Request
                # the exact integer frame for the independent oracle.
                index = int(item["seconds"] * 30)
                expected = frame_at(
                    Path(request["source"]), index / 30, frame.shape[1], frame.shape[0]
                )
                if pixels != expected:
                    raise ValueError(
                        f"MoviePy selected incorrect source pixels at frame {index}"
                    )
                hashes.append(hashlib.sha256(pixels).hexdigest())
    finally:
        for clip in clips:
            clip.close()
    write_json(
        output,
        dict(
            status="passed",
            engine="moviepy-cpu",
            latencies_us=latencies,
            frame_requests_us=sum(latencies),
            pixel_sha256=hashes,
            frame_requests=len(latencies),
            actual_decodes=None,
            seeks=None,
            cache_hits=None,
            cache_misses=None,
            decoder_opens=None,
            initial_clip_opens=request["sessions"],
            timing_scope="public get_frame only; startup/open/hash excluded",
        ),
    )


def placement_definition(width, height):
    from benchmarks.comparisons.moviepy.workloads import workload

    result = workload("layered", width, height)
    result.update(name="long-source-placements-v1", duration=3)
    result["clips"] = result["clips"][:2]
    for index, clip in enumerate(result["clips"]):
        clip.update(asset="long", start=0, duration=3, source_start=index * 6)
    return result


def placement_worker(request, output):
    from benchmarks.comparisons.moviepy.workloads import render_vestra

    result = render_vestra(
        request["definition"], Path(request["inputs"]), Path(request["media"]), "cpu"
    )
    write_json(output, result)


def run(
    directory,
    source,
    native,
    execute,
    python,
    *,
    samples=3,
    warmups=1,
    smoke=False,
    timeout=900,
):
    directory.mkdir(parents=True)
    matrix = dict(
        schema_version=1,
        status="incomplete",
        source_sha256=None,
        patterns=[],
        settings=dict(samples=samples, warmups=warmups, cache_frames=8, seed=2571),
        method="one decoder per session; only RGBA LRU budget changes; current-frame retention remains",
    )
    from benchmarks.comparisons.moviepy.comparison import digest

    matrix["source_sha256"] = digest(source)
    fixture = json.loads(source.with_suffix(".fixture.json").read_text())
    budget = fixture["width"] * fixture["height"] * 4 * 8
    from scripts.benchmark import ROOT

    try:
        for name, requests in patterns(fixture["duration"], smoke=smoke).items():
            row = dict(
                pattern=name,
                requests=requests,
                correctness="incomplete",
                samples=[],
                summary=None,
            )
            matrix["patterns"].append(row)
            base = dict(
                source=str(source.resolve()),
                requests=requests,
                sessions=2 if name == "repeated_placements" else 1,
            )

            def invoke(label, engine, cache_budget, verify):
                request = {**base, "cache_budget_bytes": cache_budget, "verify": verify}
                request_path = directory / "local" / f"{name}-{label}.json"
                request_path.parent.mkdir(exist_ok=True)
                write_json(request_path, request)
                output = directory / f"{name}-{label}.json"
                command = (
                    [str(native), str(request_path), str(output)]
                    if engine == "vestra"
                    else [
                        python,
                        str(ROOT / "scripts/collect-benchmarks.py"),
                        "--worker",
                        "cache-moviepy",
                        "--request",
                        str(request_path),
                        "--result",
                        str(output),
                    ]
                )
                measurement = execute(
                    command, directory / f"{name}-{label}.log", timeout=timeout
                )
                return dict(
                    engine=engine,
                    cache_budget_bytes=cache_budget if engine == "vestra" else None,
                    result=json.loads(output.read_text()),
                    measurement=measurement,
                )

            correctness = invoke("correctness", "vestra", budget, True)
            if correctness["result"]["exact_verified_frames"] != len(requests):
                raise ValueError("missing exact cache A/B comparisons")
            row["correctness"] = "passed"
            row["correctness_evidence"] = correctness
            row["moviepy_correctness"] = invoke(
                "moviepy-correctness", "moviepy", None, True
            )
            for sample in range(warmups + samples):
                # Alternate A/B order to reduce systematic order/thermal bias.
                order = [("off", 0), ("on", budget)]
                if sample % 2:
                    order.reverse()
                pair = dict(sample=sample, warmup=sample < warmups)
                for mode, value in order:
                    pair[mode] = invoke(f"{sample}-{mode}", "vestra", value, False)
                pair["moviepy"] = invoke(f"{sample}-moviepy", "moviepy", None, False)
                row["samples"].append(pair)
                write_json(directory / "results.json", matrix)
            measured = [s for s in row["samples"] if not s["warmup"]]
            row["summary"] = {
                f"{mode}_us": median(
                    s[mode]["result"]["frame_requests_us"] for s in measured
                )
                for mode in ("off", "on", "moviepy")
            }
        if digest(source) != matrix["source_sha256"]:
            raise ValueError("cache source changed during measurements")
        # Normal public API companion: two concurrent placements of the same
        # two-hour asset at distinct source offsets exercise actual worker pools.
        # It is separate from the isolated decoder cache A/B timings.
        from benchmarks.comparisons.moviepy.comparison import validate_output
        from scripts.benchmark import require_backend

        definition = placement_definition(128 if smoke else 640, 72 if smoke else 360)
        media = directory / "local/placements.mp4"
        request_path = directory / "local/placements-request.json"
        write_json(
            request_path,
            dict(
                definition=definition,
                inputs=str(source.parent.resolve()),
                media=str(media.resolve()),
            ),
        )
        result_path = directory / "placements-result.json"
        measurement = execute(
            [
                python,
                str(ROOT / "scripts/collect-benchmarks.py"),
                "--worker",
                "cache-placements",
                "--request",
                str(request_path),
                "--result",
                str(result_path),
            ],
            directory / "placements.log",
            timeout=timeout,
        )
        result = json.loads(result_path.read_text())
        require_backend(result, "cpu")
        matrix["public_api_placements"] = dict(
            measurement=measurement,
            result=result,
            validation=validate_output(media, definition),
            definition=definition,
            timing_scope="whole encoded worker; not isolated cache access or cache A/B",
        )
        matrix["status"] = "passed"
    finally:
        write_json(directory / "results.json", matrix)
    return matrix
