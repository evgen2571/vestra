# CPU ZoomBlur kernel optimization audit

## Scope

This audit covers only the CPU ZoomBlur effect kernel. The sample count,
authored transition parameters, bilinear interpolation, edge clamping,
premultiplied-alpha accumulation, CPU worker architecture, and WGPU path were
left unchanged.

## Environment and method

The measurements were made on Linux x86_64, Intel Core i7-8750H (6 cores, 12
threads), with Rust 1.96.1, FFmpeg 7.1.5, and the Cargo release profile. The
existing CPU renderer benchmark used automatic policy-selected 8 workers,
1280x720 output, the `examples/transitions/zoom-blur.json` fixture, 100 frames
at 30 fps, and the transition compiler's fixed 12 ZoomBlur samples. Each set
used two in-process warm operations (`warm`, `warm-2`); five independent
process runs were measured and the `warm-2` rows are reported below.

The exact before command was:

```text
VESTRA_PHASE10_BENCH=1 VESTRA_CPU_PROFILE=1 \
VESTRA_PHASE10_BENCH_START=11 VESTRA_PHASE10_BENCH_LIMIT=1 \
VESTRA_PHASE10_BENCH_OUTPUT=/tmp/vestra-zoom-baseline-N.json \
cargo test --release -p vestra phase10_release_matrix --all-features -- --nocapture
```

The after command was identical except for the output path:

```text
VESTRA_PHASE10_BENCH=1 VESTRA_CPU_PROFILE=1 \
VESTRA_PHASE10_BENCH_START=11 VESTRA_PHASE10_BENCH_LIMIT=1 \
VESTRA_PHASE10_BENCH_OUTPUT=/tmp/vestra-zoom-after-N.json \
cargo test --release -p vestra phase10_release_matrix --all-features -- --nocapture
```

## Baseline validation

The required pre-edit command passed with exit 0:

```text
./scripts/check.sh
```

It completed workspace fmt, clippy, tests, Python schema validation, and
generated-schema comparison before the edit.

## Pre-optimization evidence

The five baseline `warm-2` wall measurements were 112.607, 113.620, 113.996,
113.674, and 114.489 ms/frame. Median was 113.674 ms/frame (8.797 FPS), range
112.607–114.489 ms/frame.

The corresponding aggregate CPU timings per frame were:

| Category | Median | Range |
| --- | ---: | ---: |
| ZoomBlur | 660.826 ms | 655.527–667.873 ms |
| Source rasterization | 134.045 ms | 132.548–137.003 ms |
| Composition | 2.221 ms | 2.203–2.242 ms |

These are aggregate worker CPU milliseconds, not wall time. The profile
isolates ZoomBlur as the dominant CPU kernel in this transition workload.

## Current algorithm and implementation

Before the change, each output pixel performed 12 samples. Each sample divided
its index by `samples - 1`, selected direction-specific exposure, rebuilt the
scaled coordinate from the center and pixel ray, then called the generic
clamped bilinear sampler. The sampler recomputed image dimensions, checked up
to four neighbors, interpolated premultiplied RGB and alpha, and returned a
rounded RGBA sample. The kernel then accumulated premultiplied RGB and alpha
and normalized once per output pixel.

The CPU-only implementation now:

- precomputes the direction-specific scale for every configured sample once per
  effect pass in a bounded stack array;
- caches source dimensions once per pass;
- uses a ZoomBlur-local bilinear sampler with the same coordinate clamping,
  neighbor order, bounds checks, interpolation arithmetic, rounding, and
  premultiplied-alpha behavior;
- keeps scalar accumulation and per-pixel normalization unchanged.

No incremental coordinate stepping or approximation was introduced, so the
coordinate expressions retain their previous operation order.

## Quality and compatibility

The transition sample count is 12 before and after. Configurable authored
ZoomBlur samples remain unchanged; no quality or strength setting was reduced.
Sampling remains bilinear, edge handling remains clamped, and alpha remains
premultiplied during interpolation and accumulation. The change is confined to
`vestra-render`'s CPU ZoomBlur module. Rust public APIs, Python APIs, CLI
contracts, and project JSON/schema are unchanged.

## Correctness

A test-only copy of the former implementation compares the optimized output
byte-for-byte over transparent, semi-transparent, and opaque input; zero,
near-zero, normal, and strong radius; 2, 12, and 32 samples; center,
off-center, and corner anchors; inward, outward, and centered directions; and
a one-pixel surface. The existing golden and small-radius tests also pass.

## Performance results

| Set | Median ms/frame | Range | Median FPS |
| --- | ---: | ---: | ---: |
| Before | 113.674 | 112.607–114.489 | 8.797 |
| After | 98.806 | 97.126–99.964 | 10.121 |

The wall-time speedup is 1.150x, throughput improvement is 15.048%, and
render-time reduction is 13.080%.

The ZoomBlur aggregate CPU timing changed from 660.826 to 553.328
CPU-ms/frame, a 16.267% kernel reduction. The whole transition improved by
13.080% in the matched renderer-only workload; these are distinct measures.

## Regression matrix

The same release harness measured the post-change scenarios at 1280x720 with
automatic 8 workers. Warm-2 results were:

| Scenario | Frames | ms/frame | FPS |
| --- | ---: | ---: | ---: |
| Pipeline floor | 30 | 0.190 | 5253.24 |
| Animated transform | 100 | 15.849 | 63.10 |
| ZoomBlur transition | 100 | 98.806 | 10.12 |
| Gaussian workload | 100 | 0.171 | 5840.57 |
| Mixed realistic project (1920x1080) | 30 | 181.376 | 5.51 |

The focused Gaussian row is cache-warm after its static pass and is retained
as a shared-path regression check, not as a Gaussian performance claim.

Fresh post-change profiling ranks workload-specific CPU costs as follows:
ZoomBlur transition kernel 553.328 aggregate ms/frame; in the mixed project,
other effects and global post effects were each about 544.761 aggregate
ms/frame, Bloom/Glow about 420.492, source rasterization about 240.584, and
layer composition about 84.313. These workloads are not directly additive or
interchangeable; the next target is selected from the mixed profile below.

## Memory and parallelism

The optimization adds no heap allocation, full-frame buffer, surface, cache,
lock, worker, nested thread, or profiling timer. It uses a bounded local sample
scale array and the existing input/output surfaces. Profiling remains opt-in;
the sample loop contains no unconditional `Instant::now()` calls.

## Verification commands

Passed:

```text
cargo test -p vestra-render
cargo test -p vestra
```

The first completed with 211 passed and 2 ignored; the second completed with
82 library tests, 2 public-export tests, and 34 public-SDK tests passed. The
focused ZoomBlur test passed separately with 3 tests.

The final canonical command passed with exit 0 after the implementation:

```text
./scripts/check.sh
```

Final diff inspection found only the CPU ZoomBlur implementation, its focused
tests, and this audit document; no generated benchmark media or temporary
render outputs were added.

## Recommendation

The next single performance subphase should profile the mixed workload's
dominant non-ZoomBlur **other-effects/global-post-effect** CPU work and choose
one kernel only after fresh isolation. Gaussian, Bloom/Glow, WGPU, surface
flow, worker policy, and encoder work remain outside this subphase.
