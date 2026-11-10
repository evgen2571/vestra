#!/usr/bin/env python3
"""Render the existing CPU benchmark workloads into documentation delivery assets.

Run in the native contributor environment after just python-sync. No timings
from this script are performance evidence. --check needs only Python's stdlib.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from scripts.benchmark import build_benchmark, require_backend, suite_configuration  # noqa: E402

ASSETS = ROOT / "docs/assets/benchmarks"
PAGE = ROOT / "docs/benchmarks/workloads.md"
CATALOG = {
    "single_video": (
        "One video",
        "The forest clip measures the decode-to-encode pipeline.",
    ),
    "mixed_dynamic": (
        "Videos, shapes and a dissolve",
        "Forest and valley clips with different playback rates dissolve beneath shape, text and grouped layers.",
    ),
    "video_heavy": (
        "Concurrent video layers",
        "Three moving video placements overlap at different source offsets.",
    ),
    "combined": (
        "Combined effects",
        "Several layers and ordered effects share the same composition.",
    ),
    "masks": ("Geometric masks", "Shape coverage clips the contents of the layers."),
    "mattes": ("Track mattes", "One layer supplies coverage for another layer."),
    "nested_groups": (
        "Nested compositions",
        "Groups exercise local canvases, transforms and nested effects.",
    ),
    "particles": (
        "Particles and bloom",
        "Seeded particles exercise generation, compositing and bloom.",
    ),
    "blend_modes": (
        "Blend modes",
        "Overlapping coloured layers demonstrate the benchmark's blend operations.",
    ),
    "production_edit": (
        "Production edit",
        "A twelve-second forest and valley edit combines dissolves, colour correction, grouped titles and audio. This web preview is silent.",
    ),
    "opaque_video": (
        "Opaque video copy",
        "Two source-sized video placements include an integer-clipped overlay at full opacity.",
    ),
    "ineligible_video": (
        "Translucent video copy",
        "The same two placements use 0.9 opacity, exercising the sampling fallback.",
    ),
    "stylization_baseline": (
        "Unmodified effect control",
        "The valley clip prepared as FFV1 without an effect supplies the shared control.",
    ),
    "palette_video": (
        "Palette mapping",
        "An animated palette remaps the valley footage.",
    ),
    "oklab_palette_video": (
        "Oklab palette mapping",
        "The palette uses perceptual Oklab interpolation.",
    ),
    "dither_video": (
        "Bayer dithering",
        "Fine Bayer8 dithering maps the footage to a limited palette.",
    ),
    "blue_noise_video": (
        "Blue-noise dithering",
        "Stationary seeded blue noise replaces the ordered Bayer pattern.",
    ),
    "rgb_dither_video": ("RGB dithering", "Blue-noise dithering operates in RGB mode."),
    "hue_dither_video": (
        "Hue-aware dithering",
        "Hue-aware mapping preserves colour relationships in the limited palette.",
    ),
    "channel_dither_video": (
        "Independent channel dithering",
        "Independent channel noise changes the colour distribution.",
    ),
    "oklab_dither_video": (
        "Oklab dithering",
        "Perceptual colour distances drive the dithered palette mapping.",
    ),
    "nonuniform_dither_video": (
        "Uneven tonal stops",
        "Nonuniform palette stops redistribute the available tones.",
    ),
    "tone_dither_video": (
        "Input tone adjustment",
        "Exposure and gamma adjust the footage before dithering.",
    ),
    "detail_dither_video": (
        "Input detail enhancement",
        "Local detail enhancement precedes the same tone and dither settings.",
    ),
    "contrast_dither_video": (
        "Local contrast adjustment",
        "A wider local-detail filter changes contrast before dithering.",
    ),
    "ascii_video": (
        "ASCII characters",
        "A cached glyph atlas converts moving footage into characters.",
    ),
    "halftone_video": (
        "Halftone dots",
        "A halftone treatment represents the moving footage with dots.",
    ),
    "sort_horizontal_video": (
        "Horizontal pixel sorting",
        "Bounded horizontal sorting rearranges selected pixel spans.",
    ),
    "sort_vertical_video": (
        "Vertical pixel sorting",
        "Bounded vertical sorting rearranges selected pixel spans.",
    ),
    "crt_video": (
        "CRT treatment",
        "A seeded periodic CRT treatment processes the moving footage.",
    ),
}
EXTERNAL = {
    "basic": (
        "Basic video export",
        "The forest clip is resized and encoded through each library's public API.",
    ),
    "editing": (
        "Three-shot edit",
        "Forest, valley and tear clips overlap with 0.6-second fades, animated positioning and brightness.",
    ),
    "layered": (
        "Layered video and mask",
        "Two translucent moving overlays sit over a background; one uses a hard ellipse mask.",
    ),
    "sustained-v2": (
        "One-hour sustained render",
        "Two moving placements read the same two-hour repeated forest source. This is the first twelve seconds of an actual one-hour CPU output, not a shortened endurance run.",
    ),
}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def source_digest(path):
    # Git may check out text as CRLF on Windows; content identity is unchanged.
    return hashlib.sha256(path.read_text(encoding="utf-8").encode()).hexdigest()


def gallery(manifest):
    lines = [
        "---",
        "title: Benchmark workloads",
        "description: See the actual scenes used by Vestra's benchmark suites.",
        "hide:",
        "  - toc",
        "---",
        "",
        "<!-- Generated by scripts/render-benchmark-previews.py; edit descriptions there. -->",
        "# What the benchmarks render",
        "",
        "These are actual CPU renders of the maintained workload definitions. Video workloads use the same bundled forest, valley and tear clips as the showcases. Procedural masks, shapes, mattes and particle controls keep their diagnostic scenes.",
        "",
        "[Benchmark overview](index.md){ .md-button } [Run benchmarks](collecting.md){ .md-button }",
        "",
        "## Shared footage",
        "",
        "These three inputs are included in the checkout and also used by the showcases. Each is a silent 12-second, 1280 × 720, 30 fps clip; the collector resizes them before measurement.",
        "",
        "| Clip | Used by | Repository file |",
        "| --- | --- | --- |",
        "| Forest | Basic export, editing, layered and sustained workloads; native video timelines | [forest.mp4](https://github.com/evgen2571/vestra/blob/develop/assets/footage/forest.mp4) |",
        "| Valley | Editing, layered and effect controls; masked showcase footage | [valley.mp4](https://github.com/evgen2571/vestra/blob/develop/assets/footage/valley.mp4) |",
        "| Tear | Editing and masked overlay | [tear.mp4](https://github.com/evgen2571/vestra/blob/develop/assets/footage/tear.mp4) |",
        "",
        "Current rendering suites use version 2 (`community-v2`); the endurance graph is `sustained-v2`. Collect a fresh baseline: older generated-pattern timings use different inputs.",
        "",
        "Previews are silent 640 × 360 delivery copies at 30 fps. Original renders retain the suite resolution; fine dithering and glyph details can change when downscaled. These images explain the workloads; they are not performance measurements or output evidence for the historical report.",
        "",
        f"Captured on CPU from revision `{manifest['revision']}`; worktree modified: {str(manifest.get('working_tree_modified', True)).lower()}. The [provenance manifest](../assets/benchmarks/previews.json) identifies workload sources by content hash and records original dimensions/checksums, poster times and delivery hashes. Reproduce them with the [media instructions](../development/documentation-media.md#reproduce-benchmark-previews).",
        "",
    ]
    rendering = dict(list(CATALOG.items())[:10])
    focused = dict(list(CATALOG.items())[10:])
    for heading, entries in (
        ("Editing, layers and endurance", EXTERNAL),
        ("Rendering suites", rendering),
        ("Focused effects and controls", focused),
    ):
        lines += [f"## {heading}", ""]
        if entries is focused:
            lines += [
                '<details markdown="1">',
                "<summary>Explore the 20 focused workloads</summary>",
                "",
            ]
        lines += ['<div class="gallery-grid" markdown>', ""]
        for scenario, (title, description) in entries.items():
            row = manifest["workloads"][scenario]
            lines += [
                '<div class="gallery-card" markdown>',
                "",
                f"### {title} {{#{scenario}}}",
                "",
                f'<video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/benchmarks/{scenario}.webp" aria-label="{title}: actual benchmark output">',
                f'  <source src="../assets/benchmarks/{scenario}.mp4" type="video/mp4">',
                f'  <a href="../assets/benchmarks/{scenario}.mp4">Download the preview</a>.',
                "</video>",
                "",
                description,
                "",
                f"`{scenario}` · {row['duration']:g} s original · {row['width']} × {row['height']} · 30 fps",
                "",
                "Suites: " + ", ".join(f"`{name}`" for name in row["suites"]) + ".",
                "",
                f"[Download preview](../assets/benchmarks/{scenario}.mp4) · [Definition]({row['definition']})",
                "",
                "</div>",
                "",
            ]
        lines += ["</div>", ""]
        if entries is focused:
            lines += ["</details>", ""]
    lines += [
        "## Decoder and preparation probes",
        "",
        "Cache/access probes issue frame-request sequences, and preparation probes measure setup. They do not each produce a separate showcase video. The cache collector uses the same bundled footage as the comparison; [specialist procedures](../development/performance-probes.md) explain their scope.",
        "",
        "## Credits",
        "",
        "Video inputs are bundled under `assets/footage/` and shared with the showcases. Rendered lettering uses the repository's test font and bundled ASCII atlas; existing font notices remain with their source assets. Preview encoding and input preparation commands are maintained in the linked script and benchmark builders.",
        "",
    ]
    return "\n".join(lines)


def check():
    manifest = json.loads((ASSETS / "previews.json").read_text(encoding="utf-8"))
    assert set(manifest["workloads"]) == set(CATALOG) | set(EXTERNAL)
    for filename, expected in manifest["inputs"].items():
        assert digest(ROOT / filename) == expected, filename
    for source, expected in manifest["sources"].items():
        assert source_digest(ROOT / source) == expected, (
            f"Preview source changed: {source}"
        )
    assert PAGE.read_text(encoding="utf-8") == gallery(manifest), (
        "Regenerate the workload gallery"
    )
    for row in manifest["workloads"].values():
        for filename, expected in row["delivery_sha256"].items():
            assert digest(ASSETS / filename) == expected, filename


def deliver(original, scenario, record, manifest, *, seconds=None):
    from benchmarks.comparisons.moviepy.comparison import probe, validate_output

    info = probe(original)
    video = next(s for s in info["streams"] if s["codec_type"] == "video")
    duration = int(video["nb_read_frames"]) / 30
    seconds = seconds or duration
    poster = 0.15 if scenario == "particles" else min(seconds * 0.6, 6)
    base = ASSETS / scenario
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-i",
            str(original),
            "-t",
            str(seconds),
            "-vf",
            "scale=640:360:flags=lanczos",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "26",
            "-pix_fmt",
            "yuv420p",
            "-map_metadata",
            "-1",
            "-movflags",
            "+faststart",
            str(base.with_suffix(".mp4")),
        ],
        check=True,
    )
    validate_output(
        base.with_suffix(".mp4"), dict(width=640, height=360, fps=30, duration=seconds)
    )
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-ss",
            str(poster),
            "-i",
            str(base.with_suffix(".mp4")),
            "-frames:v",
            "1",
            "-c:v",
            "libwebp",
            "-quality",
            "85",
            "-map_metadata",
            "-1",
            str(base.with_suffix(".webp")),
        ],
        check=True,
    )
    manifest["workloads"][scenario] = dict(
        **record,
        width=video["width"],
        height=video["height"],
        duration=duration,
        backend="cpu",
        original_sha256=digest(original),
        poster_seconds=poster,
        preview_seconds=seconds,
        delivery_sha256={
            p.name: digest(p)
            for p in (base.with_suffix(".mp4"), base.with_suffix(".webp"))
        },
    )


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--check", action="store_true")
    cli.add_argument("--output", type=Path, default=ROOT / "target/benchmark-previews")
    args = cli.parse_args()
    if args.check:
        check()
        return
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    ASSETS.mkdir(parents=True, exist_ok=True)
    suites = json.loads((ROOT / "benchmarks/suites.json").read_text(encoding="utf-8"))
    executable = build_benchmark("cpu")
    sources = [
        "assets/footage/manifest.json",
        "benchmarks/suites.json",
        "benchmarks/comparisons/moviepy/workloads.py",
        "benchmarks/collector/fixtures.py",
        "benchmarks/collector/sustained.py",
    ]
    for directory in (
        "crates/vestra/benches",
        "benchmarks/projects",
        "examples/effects",
        "examples/projects",
        "examples/particles",
        "examples/compositing",
    ):
        sources += [
            str(p.relative_to(ROOT))
            for p in (ROOT / directory).rglob("*")
            if p.suffix in {".rs", ".json", ".py"}
        ]
    manifest = dict(
        revision=subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip(),
        working_tree_modified=bool(
            subprocess.check_output(["git", "status", "--porcelain"])
        ),
        executable_sha256=digest(executable),
        suite_version=suites["suite_version"],
        inputs={
            str(p.relative_to(ROOT)): digest(p)
            for p in sorted((ROOT / "assets/footage").glob("*.mp4"))
        },
        sources={p: source_digest(ROOT / p) for p in sorted(sources)},
        ffmpeg=subprocess.check_output(["ffmpeg", "-version"], text=True).splitlines()[
            0
        ],
        workloads={},
    )
    scratch = output / "native"
    scratch.mkdir()
    for scenario in CATALOG:
        memberships = [
            name
            for name in suites["suites"]
            if scenario in suite_configuration(suites, name)[1]
        ]
        name = next(n for n in memberships if "smoke" not in n)
        settings, _ = suite_configuration(suites, name)
        report = output / f"{scenario}.json"
        env = {k: v for k, v in os.environ.items() if not k.startswith("VESTRA_BENCH_")}
        env.update({f"VESTRA_BENCH_{k.upper()}": str(v) for k, v in settings.items()})
        env.update(
            VESTRA_BENCH_SCENARIO=scenario,
            VESTRA_BENCH_BACKEND="cpu",
            VESTRA_BENCH_WARMUPS="0",
            VESTRA_BENCH_SAMPLES="1",
            VESTRA_BENCH_KEEP_OUTPUT="1",
            VESTRA_BENCH_VALIDATE="1",
            VESTRA_BENCH_REPORT=str(report),
            TMPDIR=str(scratch),
            TMP=str(scratch),
            TEMP=str(scratch),
        )
        print(
            f"Rendering {scenario} ({settings['width']}×{settings['height']})",
            flush=True,
        )
        log = subprocess.check_output([str(executable)], env=env, text=True)
        (output / f"{scenario}.log").write_text(log, encoding="utf-8")
        data = json.loads(report.read_text(encoding="utf-8"))
        require_backend(data["samples"][0]["result"], "cpu")
        original_dir = Path(
            next(
                line.split("=", 1)[1]
                for line in log.splitlines()
                if line.startswith("benchmark_output=")
            )
        )
        deliver(
            original_dir / "sample-0.mp4",
            scenario,
            dict(
                suites=memberships,
                definition="https://github.com/evgen2571/vestra/blob/develop/crates/vestra/benches/animation_effects.rs",
            ),
            manifest,
        )
    from benchmarks.collector.fixtures import long_source, portable_inputs
    from benchmarks.collector.sustained import definition
    from benchmarks.comparisons.moviepy.workloads import render_vestra, workload
    from benchmarks.comparisons.moviepy.comparison import validate_output

    inputs = output / "inputs"
    portable_inputs(inputs)
    for scenario in EXTERNAL:
        print(f"Rendering {scenario}", flush=True)
        if scenario == "sustained-v2":
            long_source(inputs / "forest.mp4", inputs / "long.mp4")
            work = definition(3600)
            long_inputs = output / "sustained-inputs"
            long_inputs.mkdir()
            os.link(inputs / "long.mp4", long_inputs / "forest.mp4")
            render_inputs = long_inputs
        else:
            work, render_inputs = workload(scenario, 640, 360), inputs
        original = output / f"{scenario}.mp4"
        result = render_vestra(work, render_inputs, original, "cpu")
        require_backend(result, "cpu")
        validate_output(original, work)
        deliver(
            original,
            scenario,
            dict(
                suites=[
                    "sustained" if scenario == "sustained-v2" else "MoviePy comparison"
                ],
                definition="https://github.com/evgen2571/vestra/blob/develop/benchmarks/"
                + (
                    "collector/sustained.py"
                    if scenario == "sustained-v2"
                    else "comparisons/moviepy/workloads.py"
                ),
            ),
            manifest,
            seconds=12 if scenario == "sustained-v2" else None,
        )
    (ASSETS / "previews.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
    PAGE.write_text(gallery(manifest), encoding="utf-8")
    check()


if __name__ == "__main__":
    main()
