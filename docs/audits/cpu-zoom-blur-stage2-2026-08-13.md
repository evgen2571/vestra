# CPU ZoomBlur stage-2 exact optimization audit

## Summary

This subphase retained one simple exact CPU optimization: ZoomBlur's
specialized bilinear sampler now reads the source through a cached `as_raw()`
slice, row stride, and safe slice indexing. The first-stage scale
precomputation, sample count, worker architecture, and all rendering semantics
remain unchanged.

No coordinate stepping, opacity specialization, interior/edge sampler split,
approximation, WGPU, worker, surface-flow, Gaussian/Bloom, Color Adjust, or
Chromatic Aberration optimization was retained.

## Environment and method

Linux x86_64, Intel Core i7-8750H, Rust 1.96.1, FFmpeg 7.1.5, Cargo release
build, CPU backend, automatic 8 workers. The canonical ZoomBlur fixture was
`examples/transitions/zoom-blur.json`, rendered at 1280x720 for 100 frames;
the authored transition uses 12 ZoomBlur samples. Each benchmark process used
the existing cold, warm, and warm-2 sequence; five independent process runs
were compared using warm-2 medians.

## Pre-edit validation

Before renderer edits:

```text
./scripts/check.sh
exit 0
```

The check completed fmt, clippy, workspace tests, schema validation, and schema
comparison.

## Current accepted first-stage implementation

Before this stage, `crates/vestra-render/src/cpu/zoom_blur.rs` already cached
the direction-specific sample scales and source dimensions once per effect
execution. Its sampler preserved the canonical clamped coordinate conversion,
four-neighbor bilinear order, premultiplied-alpha interpolation, and rounded
RGBA result. The remaining inner-loop source access was
`image.get_pixel(...)` for every contributing neighbor.

## Fresh ZoomBlur baseline

The exact command was:

```text
VESTRA_PHASE10_BENCH=1 VESTRA_CPU_PROFILE=1 \
VESTRA_PHASE10_BENCH_START=12 VESTRA_PHASE10_BENCH_LIMIT=1 \
VESTRA_PHASE10_BENCH_OUTPUT=/tmp/vestra-zoom-stage2-before-N.json \
cargo test --release -p vestra phase10_release_matrix --all-features -- --nocapture
```

Resolution was 1280x720, frame count 100, worker count 8, sample count 12,
and the authored transition parameters were unchanged. There were two warmup
operations per process and five measured processes. Warm-2 raw results:

| Run | Total wall ms | Frames | ms/frame | FPS | ZoomBlur CPU-ms/frame |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 9782.574 | 100 | 97.826 | 10.222 | 551.733 |
| 2 | 9997.244 | 100 | 99.972 | 10.003 | 551.400 |
| 3 | 9873.395 | 100 | 98.734 | 10.128 | 557.977 |
| 4 | 9581.394 | 100 | 95.814 | 10.437 | 552.525 |
| 5 | 9930.997 | 100 | 99.310 | 10.069 | 552.291 |

Median: 9873.395 total wall ms, 98.734 ms/frame, 10.128 FPS, range
9581.394–9997.244 total wall ms (95.814–99.972 ms/frame), and 552.291
aggregate ZoomBlur CPU-ms/frame. Source rasterization median was 135.1
CPU-ms/frame and layer composition median was 2.24 CPU-ms/frame.

## Candidate optimizations

### Safe raw RGBA source access — retained

The pass caches `source.as_raw()` and `width * 4` once, then uses checked slice
indexing for each valid neighbor. The coordinate arithmetic, bounds checks,
neighbor order, alpha conversion, accumulation order, and rounding were not
changed. Focused reference tests remained byte-identical.

After five release/profile runs, warm-2 results were:

| Run | Total wall ms | Frames | ms/frame | FPS | ZoomBlur CPU-ms/frame |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 9580.264 | 100 | 95.803 | 10.438 | 533.142 |
| 2 | 9698.749 | 100 | 96.987 | 10.311 | 540.620 |
| 3 | 9630.430 | 100 | 96.304 | 10.384 | 534.531 |
| 4 | 9609.296 | 100 | 96.093 | 10.407 | 532.028 |
| 5 | 9767.668 | 100 | 97.677 | 10.238 | 544.270 |

Median: 9630.430 total wall ms, 96.304 ms/frame, 10.384 FPS, range
9609.296–9767.668 total wall ms (96.093–97.677 ms/frame), and 534.531
aggregate ZoomBlur CPU-ms/frame. This simple change was retained.

### Incremental coordinate stepping — rejected before implementation

Stepping a ray by a delta would change floating-point operation order and can
change floor/clamp neighbor selection. Since byte identity is mandatory, the
existing per-sample coordinate formula was preserved and no benchmarked
production variant was kept.

### Interior/edge sampler split — rejected before implementation

The current canonical edge clamp is part of every sample's exact behavior. A
second large sampler would duplicate fragile bilinear arithmetic; no cheap
classification was demonstrated that justified the complexity for this stage.

### Opaque specialization — rejected before implementation

The pass has no cheap existing proof that its complete input is opaque. A full
image opacity scan would add work similar to the prior rejected Gaussian
experiment, so no scan or alternate alpha path was introduced.

## Retained implementation and semantics

The production change is limited to `crates/vestra-render/src/cpu/zoom_blur.rs`:

- cache raw RGBA bytes and row stride once per effect pass;
- use safe checked slice indexing for four bilinear neighbors;
- retain the first-stage scale cache and all existing scalar accumulation.

Confirmed unchanged: sample count (12 in the canonical fixture), coordinate
formula, bilinear interpolation, edge clamping, premultiplied-alpha behavior,
accumulation order, and output rounding.

## Byte-equivalence

The existing former-implementation reference compares optimized output to the
accepted first-stage behavior. It covers 2, 12, and 32 samples; inward,
outward, and centered directions; zero, near-zero, representative, and strong
radii; center, off-center, and corner anchors; opaque, semi-transparent,
transparent, and mixed-alpha input; fractional sampling and edges; and a 1x1
image. The centered golden and small-radius behavior tests also pass. All
comparisons are byte-for-byte.

## Memory and parallelism

No per-pixel or per-sample heap allocation, temporary image, proportional
array, full-frame surface, global state, lock, nested parallelism, or new
thread was added. The raw slice and stride are borrowed pass-local values.
Existing worker-local rendering, random-access frames, and opt-in coarse
profiling remain unchanged.

## Performance delta

The retained implementation changes the primary ZoomBlur transition from
98.734 to 96.304 ms/frame and 10.128 to 10.384 FPS:

- speedup: 1.025x;
- throughput improvement: 2.53%;
- frame-time reduction: 2.46%.

ZoomBlur aggregate CPU timing changes from 552.525 to 534.531 CPU-ms/frame:

- kernel reduction: 3.22%.

Kernel and whole-render deltas are separate measurements.

## Mixed workload impact

The canonical mixed fixture is 1920x1080, 30 frames, 8 workers. It contains no
ZoomBlur pass: every profile reports `zoom_blur_ns=0`. Repeated warm-2 totals
were:

| Run | Before total wall ms | Before ms/frame | Before FPS | After total wall ms | After ms/frame | After FPS |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 4727.133 | 157.571 | 6.346 | 4746.680 | 158.223 | 6.320 |
| 2 | 4731.462 | 157.715 | 6.341 | 4733.431 | 157.781 | 6.338 |
| 3 | 4711.580 | 157.053 | 6.393 | 4692.396 | 156.413 | 6.393 |
| 4 | 4812.441 | 160.415 | 6.234 | 4679.561 | 155.985 | 6.411 |
| 5 | 4709.775 | 156.993 | 6.370 | 4719.657 | 157.322 | 6.356 |

Before median was 4727.133 total wall ms / 157.571 ms/frame / 6.346 FPS;
after median was 4719.657 total wall ms / 157.322 ms/frame / 6.356 FPS. The
0.16% wall difference is noise in a workload that does not execute ZoomBlur;
the change has no measured mixed-kernel impact.

Fresh after mixed exclusive CPU ranking uses a 1116.2 CPU-ms/frame denominator
of non-overlapping concrete pass timers. ZoomBlur and Chromatic Aberration are
0 because this fixture does not exercise them; inclusive parent timers such as
`global_post_effect`, `effect_execution`, and `bloom_glow` are excluded.

| Exclusive work | CPU-ms/frame | Share |
| --- | ---: | ---: |
| Bloom Gaussian H/V | 294.3 | 26.4% |
| Source rasterization | 238.5 | 21.4% |
| Sharpen Gaussian H/V | 221.0 | 19.8% |
| Bloom composite | 92.2 | 8.3% |
| Vignette | 91.0 | 8.2% |
| Layer composition | 84.4 | 7.6% |
| Sharpen Unsharp composite | 39.0 | 3.5% |
| Bloom extraction | 46.2 | 4.1% |
| Color Adjust | 7.3 | 0.7% |
| Surface copy | 2.3 | 0.2% |

Rounded shares sum to 100.2% due to presentation rounding. This ranking is
the concrete non-overlapping work only; inclusive parent fields are excluded.

## Regression matrix

Post-change release warm-2 checks used the existing phase-10 harness:

| Workload | Frames | ms/frame | FPS | Result |
| --- | ---: | ---: | ---: | --- |
| Simple/static pipeline floor | 30 | 0.194 | 5155.61 | pass |
| Animated transform | 100 | 16.708 | 59.85 | pass |
| ZoomBlur transition | 100 | 96.387 | 10.37 | pass |
| Gaussian/Bloom shared-path check (global-post fixture) | 30 | 0.174 | 5733.15 | pass |
| Mixed | 30 | 151.775 | 6.59 | pass |
| Color Adjust focused | 100 | 20.273 | 49.33 | pass |

The focused Gaussian/Bloom CPU test suite also passed; no Gaussian/Bloom
production source changed.

## Tests and parity

Passed:

```text
cargo test -p vestra-render
214 passed; 2 ignored

cargo test -p vestra
82 library passed; 2 public-export passed; 34 public-SDK passed
```

The `vestra-render` suite includes CPU/WGPU parity tests. They passed where an
adapter was available and preserved the repository's normal skip behavior
where unavailable.

## Final validation

The final canonical validation is recorded after this audit and the retained
implementation are in place:

```text
./scripts/check.sh
exit 0
```

## Compatibility and scope

No Rust public API, Python API, CLI contract, project JSON/schema, crate
boundary, worker policy, or WGPU implementation changed. Gaussian/Bloom source,
Color Adjust LUT, Chromatic Aberration, source rasterization, composition,
surface flow, encoder behavior, and quality/sample count remain unchanged.

## Recommendation

ZoomBlur remains a dominant isolated transition kernel, but this simple exact
raw-access pass has reached a modest gain. The one next performance subphase
should be **CPU Chromatic Aberration hot-path optimization**, selected from the
fresh broader ranking once a fixture that actually exercises it is profiled.
