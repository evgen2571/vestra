#!/usr/bin/env python3
"""Measure Python/native boundary calls in an installed wheel (not engine throughput)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import platform
import statistics
import sys
import timeit

import vestra
from vestra import _native


def main() -> None:
    project = vestra.Project(size=(32, 32), fps=30, duration=1)
    for index in range(16):
        project.root.add(vestra.Color("#4080c0"), name=f"layer-{index}")
    snapshot = project.snapshot()
    data = snapshot.to_dict()
    prepared = vestra.Editor().prepare(
        snapshot, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU)
    )
    frame = prepared.render_frame_number(0)
    assert len(frame.to_bytes()) == 32 * 32 * 4
    assert frame.to_bytes()[:4] == bytes.fromhex("4080c0ff")
    assert vestra.ProjectSnapshot.from_dict(data).to_dict() == data
    assert prepared.preparation_report.selected_backend == vestra.BackendKind.CPU

    workloads = {
        "native_version": (vestra.native_version, 1_000_000),
        "frame_width": (lambda: frame.width, 1_000_000),
        "frame_to_bytes": (frame.to_bytes, 1_000_000),
        "project_from_dict_16_layers": (
            lambda: vestra.ProjectSnapshot.from_dict(data),
            1_000,
        ),
        "project_to_dict_16_layers": (snapshot.to_dict, 1_000),
        "prepared_cpu_frame_32x32_reused": (
            lambda: prepared.render_frame_number(0),
            100_000,
        ),
    }
    results = {}
    for name, (operation, iterations) in workloads.items():
        for _ in range(100):
            operation()
        samples = timeit.repeat(operation, number=iterations, repeat=7)
        results[name] = {
            "iterations_per_sample": iterations,
            "seconds": samples,
            "median_ns_per_call": statistics.median(samples) * 1e9 / iterations,
        }
    native_path = Path(_native.__file__)
    print(
        json.dumps(
            {
                "python": sys.version,
                "platform": platform.platform(),
                "vestra_version": vestra.__version__,
                "native_filename": native_path.name,
                "native_sha256": hashlib.sha256(native_path.read_bytes()).hexdigest(),
                "samples": 7,
                "warmup_calls": 100,
                "results": results,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
