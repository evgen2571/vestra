# CPU Chromatic Aberration exact hot-path optimization audit

## Summary

The CPU Chromatic Aberration hot path retained one exact optimization in
`vestra-render`: it now samples only the selected red or blue channel while
preserving the existing premultiplied-alpha bilinear calculation, and reads
source pixels through a borrowed cached RGBA slice and row stride. No effect
parameters, quality, public API, WGPU implementation, worker policy, or
surface topology changed.

## Environment and pre-edit validation

Measurements used the existing Linux x86_64 host, Cargo release builds, CPU
backend, automatic 8 workers, preview quality, and the existing Phase 10
NullSink harness. The canonical pre-edit validation was:

```text
./scripts/check.sh
exit 0
```

It completed fmt, clippy, workspace tests, schema validation, and schema
comparison before renderer edits.

The prior ZoomBlur stage-2 audit was corrected before renderer edits. Its
sorted before samples have median `552.291` CPU-ms/frame, not `552.525`, and
the `534.531` after median therefore represents a `3.22%` kernel reduction.
Benchmark source data was not changed.

## Fixture distinction and active workload

The existing short `mixed` Phase 10 row is a 30-frame, 1920x1080 slice of
`examples/projects/effects-ready-v1.json`. Its frames end before the incoming
clip begins at 2.5 seconds, so its Chromatic Aberration counter is zero and it
is not a valid Chromatic Aberration ranking fixture.

The longer `long-combined` row uses the same project for 180 frames at
1920x1080. It reaches the incoming clip and the authored Chromatic Aberration
section. A dedicated `chromatic-focused` row reuses that incoming clip,
starts it at frame zero, removes unrelated effects/transitions, renders 100
frames at 1280x720, and bypasses the static cache so every measured frame
executes the effect. Its unchanged parameters are amount `2`, angle `0°`,
preview quality, CPU backend, and 8 workers.

## Fresh focused baseline

The focused command was the existing release matrix with
`VESTRA_CPU_PROFILE=1`, two in-process warmups (`warm`, `warm-2`), and five
independent processes. Warm-2 raw wall samples were:

| Run | Total wall ms | Frames | ms/frame | FPS |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 234.338 | 100 | 2.343 | 426.734 |
| 2 | 249.559 | 100 | 2.496 | 400.707 |
| 3 | 233.563 | 100 | 2.336 | 428.150 |
| 4 | 235.434 | 100 | 2.354 | 424.748 |
| 5 | 233.622 | 100 | 2.336 | 428.042 |

The median is `234.338 ms` total, `2.343 ms/frame`, and `426.734 FPS`; the
total range is `233.563–249.559 ms` (`2.336–2.496 ms/frame`). The focused
Chromatic Aberration profiler samples were `104.390`, `104.645`, `104.469`,
`106.614`, and `121.363` ms of aggregate CPU duration for the 100-frame
operation. The median is `104.645 ms` aggregate, or `1.04645 CPU-ms/frame`
after dividing by 100.

## Fresh long-combined baseline

The long-combined workload used the same project at 1920x1080 for 180 frames,
with two warmups and five independent measured runs. Warm-2 raw wall samples:

| Run | Total wall ms | Frames | ms/frame | FPS |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 44814.988 | 180 | 248.972 | 4.017 |
| 2 | 44523.678 | 180 | 247.354 | 4.043 |
| 3 | 44958.245 | 180 | 249.768 | 4.004 |
| 4 | 44911.251 | 180 | 249.507 | 4.008 |
| 5 | 45216.939 | 180 | 251.205 | 3.981 |

The median is `44911.251 ms` total, `249.507 ms/frame`, and `4.008 FPS`; the
total range is `44523.678–45216.939 ms` (`247.354–251.205 ms/frame`).

Fresh exclusive aggregate CPU timings are divided by 180 frames. The
denominator is `1922.490 CPU-ms/frame`, the sum of these non-overlapping
concrete timers; inclusive `effect_execution`, `global_post_effect`,
`bloom_glow`, and `sharpen` are excluded.

| Exclusive work | CPU-ms/frame | Share |
| --- | ---: | ---: |
| ZoomBlur | 672.912 | 35.0% |
| Source rasterization | 290.733 | 15.1% |
| Sharpen Gaussian H/V | 234.995 | 12.2% |
| Chromatic Aberration | 197.944 | 10.3% |
| Bloom Gaussian H/V | 178.933 | 9.3% |
| Layer composition | 112.525 | 5.9% |
| Vignette | 95.522 | 5.0% |
| Bloom additive composite | 56.173 | 2.9% |
| Sharpen Unsharp composite | 41.600 | 2.2% |
| Bloom highlight extraction | 28.548 | 1.5% |
| Color Adjust | 10.283 | 0.5% |
| Surface copy | 2.319 | 0.1% |

The displayed shares sum to approximately 100% (100.0% before presentation
rounding). Chromatic Aberration is a meaningful independent target at
`197.944 CPU-ms/frame`, fourth in this fresh exclusive ranking.

## Current algorithm and hot loop

For each destination pixel, the former CPU path computed one displacement
vector from `amount` and `angle_degrees`, called the canonical clamped
premultiplied-alpha RGBA bilinear sampler at the negative displacement and
again at the positive displacement, took red from the first result and blue
from the second, and copied centered green and source alpha. Each selected
channel therefore paid for three-channel interpolation and construction of a
temporary RGBA value twice.

The exact retained path keeps the same angle conversion, displacement
coordinates, pixel-center convention, clamp bounds, four-neighbor order,
alpha weights, premultiplied RGB calculation, threshold, rounding, and
channel recombination. It computes only the requested channel's
premultiplied accumulator plus the shared alpha accumulator, using safe raw
slice offsets.

## Candidate optimizations

| Idea | Correctness | Performance | Decision |
| --- | --- | --- | --- |
| Channel-specific exact bilinear sampling with cached raw RGBA slice and row stride | Reference output byte-identical across fractional coordinates, angles, alpha cases, edges, 1x1, and transparent input | Focused kernel median `104.645→66.151 ms` aggregate over 100 frames, or `1.04645→0.66151 CPU-ms/frame`; `36.79%` reduction | Retained |
| Incremental coordinate stepping | Would alter floating-point operation order and could change floor/clamp neighbor selection; no exact implementation was retained | Not benchmarked because byte identity is a hard requirement | Rejected |
| Interior/edge split | Would duplicate the canonical clamp-sensitive sampler and adds substantial complexity without a demonstrated exact fast classifier | Not benchmarked before implementation | Rejected |

No approximation, nearest-neighbor path, reduced precision, full-frame
temporary, unsafe access, or shared-helper rewrite was used.

## Selected implementation and semantics

Production changes are confined to `crates/vestra-render/src/cpu/chromatic.rs`:

- cache `as_raw()`, dimensions, and row stride once per effect pass;
- use checked safe slice indexing for four contributing neighbors;
- accumulate only the selected channel while retaining alpha-aware math;
- leave centered green, source alpha, geometry, and output recombination unchanged.

Confirmed unchanged: channel offsets, coordinate math, bilinear semantics,
premultiplied-alpha behavior, edge/clamp behavior, floating-point types,
neighbor order, rounding, effect parameters, and identity handling.

## Byte-equivalence

The test-only former-path reference calls the canonical `sample_edge` RGBA
implementation and compares complete output images byte-for-byte. Coverage
includes zero, near-zero, representative, and strong amounts; center and
off-axis angles; opaque, semi-transparent, transparent, and mixed-alpha
pixels; mixed colors; fractional coordinates; out-of-bounds clamping; and
1x1 and small images. The focused CPU test target passed both equality tests.

## Memory, parallelism, and architecture

The implementation adds no per-pixel allocation, temporary image, full-frame
buffer, global state, lock, thread, nested parallelism, or worker-policy
change. The borrowed source slice and scalar sampler state are pass-local.
Rendering remains random-access and profiling remains opt-in. The fast path
stays inside the CPU renderer; core effect semantics, WGPU, Python, CLI, and
public Rust APIs are untouched.

## Before and after focused benchmark

After warm-2 raw wall samples were:

| Run | Total wall ms | Frames | ms/frame | FPS |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 195.293 | 100 | 1.953 | 512.052 |
| 2 | 195.865 | 100 | 1.959 | 510.556 |
| 3 | 195.280 | 100 | 1.953 | 512.086 |
| 4 | 201.703 | 100 | 2.017 | 495.778 |
| 5 | 196.540 | 100 | 1.965 | 508.801 |

The median is `195.865 ms` total, `1.959 ms/frame`, and `510.556 FPS`; the
total range is `195.280–201.703 ms` (`1.953–2.017 ms/frame`).

Focused Chromatic Aberration profiler samples were `66.032`, `66.203`,
`66.731`, `66.151`, and `66.040` ms of aggregate CPU duration for the
100-frame operation. The median is `66.151 ms` aggregate, or `0.66151
CPU-ms/frame` after dividing by 100.

## Performance delta

Focused wall speedup is `1.196x`, throughput improvement `19.64%`, and
frame-time reduction `16.42%` (`2.343→1.959 ms/frame`). The focused kernel
delta is `104.645→66.151 ms aggregate`, equivalent to `1.04645→0.66151
CPU-ms/frame`, a `36.79%` reduction.

The 180-frame long-combined after samples were:

| Run | Total wall ms | Frames | ms/frame | FPS |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 44253.527 | 180 | 245.853 | 4.067 |
| 2 | 43243.301 | 180 | 240.241 | 4.162 |
| 3 | 44079.788 | 180 | 244.888 | 4.084 |
| 4 | 44320.873 | 180 | 246.227 | 4.061 |
| 5 | 43613.538 | 180 | 242.297 | 4.127 |

The long-combined median is `44079.788 ms` total, `244.888 ms/frame`, and
`4.084 FPS`; the total range is `43243.301–44320.873 ms`
(`240.241–246.227 ms/frame`). Chromatic Aberration profiler samples were
`127.114`, `127.768`, `126.230`, `127.028`, and `126.797` CPU-ms/frame after
normalizing each 180-frame aggregate counter. The median is `127.028
CPU-ms/frame`, which represents `22,865.040 ms` aggregate. The corresponding
pre-edit `197.944 CPU-ms/frame` represents `35,629.920 ms` aggregate. The
long-combined kernel reduction is `35.83%`; whole-render frame time improves
`1.85%` and throughput improves
`1.89%`. Kernel and whole-render deltas are separate.

## Regression matrix and tests

The existing release Phase 10 harness was used for simple/static pipeline
floor, animated transform, ZoomBlur transition, Color Adjust focused, the
short mixed row, the focused Chromatic Aberration row, and the long combined
row. The short mixed row remains an inactive Chromatic Aberration regression
check, not a performance baseline. The final warm-2 regression measurements
were:

| Workload | Frames | Resolution | ms/frame | FPS | Result |
| --- | ---: | --- | ---: | ---: | --- |
| Simple/static pipeline floor | 30 | 1280x720 | 0.188 | 5310.91 | pass |
| Animated transform | 100 | 1280x720 | 17.041 | 58.68 | pass |
| ZoomBlur transition | 100 | 1280x720 | 94.353 | 10.60 | pass |
| Chromatic Aberration focused | 100 | 1280x720 | 1.959 | 510.56 | pass |
| Short mixed (inactive CA) | 30 | 1920x1080 | 160.485 | 6.23 | pass |
| Long combined | 180 | 1920x1080 | 244.888 | 4.08 | pass |
| Color Adjust focused | 100 | 1280x720 | 20.357 | 49.12 | pass |

The focused CPU tests passed:

```text
cargo test -p vestra-render cpu::chromatic --all-features
2 passed; 0 failed
```

The full `vestra-render` and `vestra` test suites, including existing CPU/WGPU
parity coverage, are required in final validation. Gaussian/Bloom production
code and shared sampling helpers were not changed.

## Audit arithmetic and remaining ranking

Every wall table distinguishes total wall time, frame count, ms/frame, and
FPS. The long-combined post-change exclusive ranking uses the same
`1890.124 CPU-ms/frame` non-overlapping denominator:

```text
ZoomBlur                 683.476
Source rasterization     298.802
Sharpen Gaussian H/V     240.625
Bloom Gaussian H/V       184.247
Chromatic Aberration     127.028
Layer composition         115.493
Vignette                   97.860
Bloom additive composite  57.958
Sharpen Unsharp composite 43.464
Bloom highlight extract   28.122
Color Adjust               10.818
Surface copy                2.231
```

The rounded shares sum to approximately 100%. ZoomBlur remains the dominant
fresh concrete CPU kernel. The one recommended next performance subphase is
**CPU source rasterization**, subject to a new exact focused audit; no second
Chromatic Aberration micro-pass is scheduled.

## Scope and compatibility

No Rust public API, Python API, CLI contract, project JSON/schema, crate
boundary, WGPU code, worker architecture, worker policy, ZoomBlur production
code, Gaussian/Bloom production code, Color Adjust code, surface-copy path,
quality setting, or unrelated feature/refactor changed.
