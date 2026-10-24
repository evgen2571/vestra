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
just python-sync
uv pip install --python .venv/bin/python -r benchmarks/comparisons/moviepy/requirements.txt

# Fresh directory; defaults to ~/drive/Assets. No downloads or replacements.
.venv/bin/python scripts/benchmark.py moviepy prepare
# Optional: --assets /path/to/Assets --inputs target/my-moviepy-inputs

# Viability only; these timings are not performance evidence.
.venv/bin/python scripts/benchmark.py moviepy run --width 320 --height 180 \
  --warmups 0 --samples 1 --output target/moviepy-smoke

# All three CPU workloads, one warmup + five measured samples per engine.
VESTRA_WGPU_BACKEND=vulkan .venv/bin/python scripts/benchmark.py moviepy run \
  --output target/moviepy-cpu

# Separate strict hardware run; select the API found by adapter discovery.
just wgpu-list
VESTRA_WGPU_BACKEND=vulkan .venv/bin/python scripts/benchmark.py moviepy run \
  --engines hardware-wgpu --output target/moviepy-hardware

# Decode representative boundary/source frames, compare and create local sheets.
.venv/bin/python scripts/benchmark.py moviepy verify \
  target/moviepy-cpu target/moviepy-hardware --output target/moviepy-visuals
.venv/bin/python scripts/benchmark.py moviepy report \
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

Do not run `uv sync` between installing the comparison dependencies and rendering:
it removes these optional tools. The explicit `.venv/bin/python` commands retain
the installed development package. Package versions and the native extension
hash are recorded; MoviePy's distribution is 2.2.1 even though its internal
`__version__` string says 2.1.2.

## Inputs and equivalent operations

Preparation uses the first 12 seconds of forest, valley and tear, center-cropped
to 1280×720, CFR 30 fps, silent H.264/yuv420p, CRF 18 / veryfast, two encoder
threads, bicubic FFmpeg scaling and stripped metadata/bitexact flags. Resizing
from these shared inputs is part of each engine's workload. `manifest.json`
records exact commands, source/prepared SHA-256, probes and FFmpeg version.
Identical bytes across different FFmpeg builds are not promised. Runs reject
changed prepared files; missing/short/undecodable originals fail with an error.
Originals remain read-only. Preparation is never part of the timing comparison.

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
a sum of independent process peaks or GPU VRAM. CPU utilization uses Unix waited
child CPU time including reaped descendants, with one core = 100%; it may exceed
100%. Unsupported/missing metrics remain JSON null. Psutil is optional to the
shared runner but installed by this comparison's requirements for RSS evidence.

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

Generated MP4s, PNGs, prepared files, raw records and logs stay under ignored
`target/`; do not commit them. Reports are machine-local and may contain source
paths in probe metadata. Review before sharing. Collection footage licensing
and permission to redistribute remain with the collection owner.

## Collection inspection

The initial collection was probed before selecting footage. Font archives were
inspected for font members and OFL notices. Unused items remain untouched.

| Original | Inspected media | Use |
| --- | --- | --- |
| `forward-fly-above-snow-forest.mp4` | H.264 2704×1520, 24000/1001 fps, 19.394 s; AAC 48 kHz stereo | Forest input; audio removed |
| `forward-fly-through-road-to-snow-valley.mp4` | H.264 3840×2160, 25 fps, 31.240 s; no audio | Valley input |
| `dropping-tear.mp4` | H.264 3840×2160, 24 fps, 16.583 s; no audio | Masked tear input |
| `static-forest-dynamic-starry-sky.mp4` | H.264 3840×2160, 24 fps, 5.042 s; no audio | Too short for an unlooped 10 s shot |
| `stars-on-black-background.jpg` | JPEG 7001×4001, YUV444 | Unused |
| `city-mask.png` | PNG 3150×1816, RGBA | Unused; hard ellipse gives a closer engine match |
| `Cinzel.zip`, `Lobster_Two.zip`, `Playwrite_AR_Guides.zip` | TrueType fonts and OFL notices | First two used by Code → Video only |
| `＂Fragments＂ by @AERHEAD.mp3` | MP3 329.839 s, 48 kHz stereo, embedded PNG | Unused |
| `＂IN MEMORIAM＂ by Onycs.mp3` | MP3 189.243 s, 48 kHz stereo, embedded PNG | Unused |

The new projects do not depend on existing `examples/showcase/assets/` media.
Those files are still used by the old showcases, so none is established as
unused. If the old showcases are retired later, their city clips, landscape,
synth soundtrack, font and rendered MP4/poster/preview derivatives are cleanup
candidates after checking references, including the current root README. Leave
the small `examples/assets/` fixtures used by canonical examples/tests in place.
