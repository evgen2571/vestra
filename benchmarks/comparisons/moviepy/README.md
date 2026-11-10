# Vestra and MoviePy v2 comparison

This extends [the existing benchmark runner](../../../scripts/benchmark.py).
It compares three silent video timelines through the normal Python APIs, with
1920×1080 / 30 fps output. It measures complete worker processes, including
Python startup/imports, project construction, media decoding, rendering,
libx264 encoding, finalization and publication. Input preparation and output
verification happen outside the measured intervals. These are editing workloads,
not shader microbenchmarks or evidence of universal performance differences.

## Reproduce

From the repository root, enter the [development environment](../../../CONTRIBUTING.md).
On Nix, use the comparison shell: it adds the C++/zlib runtime for NumPy wheels
and the Vulkan/GL loaders for adapter discovery, without forcing a software GPU.
On other systems use the equivalent native environment with FFmpeg/FFprobe.

```bash
nix develop .#comparison
uv sync --locked --extra benchmark --extra dev

# Shared forest/valley/tear inputs are already bundled in assets/footage/.

# Viability only; these timings are not performance evidence.
uv run --locked --extra benchmark --extra dev python scripts/benchmark.py moviepy run --width 320 --height 180 \
  --warmups 0 --samples 1 --output target/moviepy-smoke

# All three CPU workloads, one warmup + five measured samples per engine.
VESTRA_WGPU_BACKEND=vulkan uv run --locked --extra benchmark --extra dev python scripts/benchmark.py moviepy run \
  --output target/moviepy-cpu

# Separate strict hardware run; select the API found by adapter discovery.
just wgpu-list
VESTRA_WGPU_BACKEND=vulkan uv run --locked --extra benchmark --extra dev python scripts/benchmark.py moviepy run \
  --engines hardware-wgpu --output target/moviepy-hardware

# Decode representative boundary/source frames, compare and create local sheets.
uv run --locked --extra benchmark --extra dev python scripts/benchmark.py moviepy verify \
  target/moviepy-cpu target/moviepy-hardware --output target/moviepy-visuals
uv run --locked --extra benchmark --extra dev python scripts/benchmark.py moviepy report \
  target/moviepy-cpu target/moviepy-hardware \
  --verification target/moviepy-visuals/metrics.json > target/moviepy-report.md
```

Use a fresh output directory for each command. Add `--scenarios basic`,
`--scenarios editing`, or `--scenarios layered` to run one workload;
`--engines vestra-cpu moviepy-cpu` is the default. `--inputs` selects prepared
inputs for run/verify. `--timeout` bounds each worker (default 900 seconds).
`--warmups` and `--samples` default to 1 and 5. Reduced settings are smoke checks.
Keep all benchmark sources unchanged throughout captures, run serially, finish
compilation first, and avoid other heavy workloads. Use the same graphics API
environment for CPU and hardware captures that will be verified together.
The shell exposes loaders, not device permissions/drivers; unavailable hardware
or a software/unknown adapter is a failed hardware run, never a CPU replacement.

Keep `--extra benchmark` on sync/run commands so MoviePy remains installed.
The locked benchmark extra replaces manual requirements installation. Package
versions and the native extension hash are recorded; MoviePy's distribution is
2.2.1 even though its internal version string says 2.1.2. For the portable benchmark workflow,
use [the collector](../../COLLECTING.md).

## Inputs and equivalent operations

Preparation uses the first 12 seconds of forest, valley and tear, center-cropped
to 1280×720, CFR 30 fps, silent H.264/yuv420p, CRF 23 / slow, two encoder
threads, bicubic FFmpeg scaling and stripped metadata/bitexact flags. Resizing
from these shared inputs is part of each engine's workload. `manifest.json`
records checksums, frame counts and encoding settings. Runs reject changed
prepared files. Preparation is never part of the timing comparison.

By default, run/verify use `assets/footage/`, shared with the showcases. To
prepare a different local collection, use `moviepy prepare --assets DIRECTORY
--inputs target/my-moviepy-inputs` and pass that `--inputs` path to run/verify.
Use a fresh output directory and collect a separate baseline for changed inputs.

| Scenario | Timeline and operations |
| --- | --- |
| `basic` | Forest 0–10 s, resize to the output canvas, encode. |
| `editing` | Forest 0–4.4 s, valley 3.8–8.2 s, tear 7.6–12 s. Incoming linear opacity ramps over 0.6 s, outgoing clip remains opaque. Each shot has 1.05× scale, horizontal center moving .49→.51 in local time, and additive brightness +.03 (7.65/255). |
| `layered` | Forest 0–10 s. Valley 1–9 s at .5 scale, .85 opacity, x .30→.38/y .5. Tear 3–9 s at .38 scale, .7 opacity, x .72→.66/y .52, hard ellipse mask with axes 90% of its frame. |

The definition in `workloads.py` drives both implementations. Layers use normal
source-over composition. MoviePy uses native v2 `subclipped`, `resized`,
`with_position`, `with_opacity`, `LumContrast`, `CrossFadeIn`, `ImageClip` masks
and `CompositeVideoClip`. Basic uses a direct clip; transforms use one resize
at the final layer size. Vestra uses its public Project/Video/keyframe/effect/
mask APIs. Both encode with the same system FFmpeg, libx264 CRF 23 / veryfast /
yuv420p, no audio; encoder/decode concurrency retains each library's defaults.
Hardware WGPU still uses CPU decoding and libx264 encoding.

Resamplers, coordinate rounding, ellipse edge coverage and brightness rounding
are different. MoviePy's Pillow resizing and Vestra's raster/GPU sampling need
not match pixels or encoded sizes. Verification keeps full-size raw metrics and
differences plus a low-frequency comparison and layer-region checks. Inspect
the local contact sheets and per-frame metrics before sharing results; a global
image average alone cannot prove a small overlay is present. No public media,
screenshots, performance table or README redesign is part of this milestone.

## Evidence and failures

`comparison.json` retains environment, package/tool versions, CPU/affinity,
revision/source/native/FFmpeg fingerprints, prepared manifest, workload settings,
requested/actual backend, adapter/classification, warmups, individual samples,
output validation, median/min/max/standard deviation and stage timings. The
Vestra counters distinguish preparation, decode, frame work, encoder writes and
finalization; overlapping stages do not add to wall time. MoviePy exposes
construction and `write_videofile` timing here, not equivalent internal stages.
The report labels these scopes; use internal Vestra benchmarks for stage isolation.

RSS is a 20 ms sample of the simultaneous renderer-plus-descendant process tree,
including FFmpeg; it is a lower bound and double-counts shared pages. It is not
a sum of independent process peaks or GPU VRAM. CPU utilization uses sampled
observed-tree CPU time on each platform, with one core = 100%; short-lived
unobserved descendants can be missed and utilization may exceed 100%.
Unsupported/missing metrics remain JSON null. Psutil is optional to the shared
runner but installed by the locked benchmark extra for RSS evidence.

Every output is probed for resolution, rational FPS, duration, decoded frame
count, silent H.264/yuv420p and is fully decoded with FFmpeg's error gate. Warmups
are validated too. Worker failures/timeouts or invalid media leave a failed
capture with logs and prohibit a successful summary. Source/input changes reject
the capture. Verification samples the first measured output per engine at exact
frame indices around transitions, animated placements, overlay starts/ends and
the final frame; clear isolated shots also get independent source-frame checks.
Other repetitions receive structural/full-decode validation, not exhaustive
pixel comparisons. Reports without matching successful visual evidence are
explicitly unverified.

Generated renders, PNGs, raw records and logs stay under ignored `target/`.
Only the compact shared inputs in `assets/footage/` are committed. Reports are
machine-local and may contain paths in probe metadata.
