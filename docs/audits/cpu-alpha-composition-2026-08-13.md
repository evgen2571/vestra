# CPU alpha composition hot-path optimization audit

## Summary

One exact CPU composition optimization was retained in `vestra-render`:
normal blend-mode dispatch is selected once per surface pass, avoiding the
per-pixel normal-mode match. A transparent effective source-alpha return was
also retained in the shared source-over helper. Both preserve the existing
straight-alpha `f64` arithmetic byte-for-byte. No raster sampler, effect,
WGPU, worker, surface-flow, public API, or schema work was included.

## Baseline validation

Before edits:

```text
./scripts/check.sh
exit 0
```

This completed format, workspace clippy, all-feature workspace tests, schema
validation, and generated-schema comparison.

## Composition architecture

`crates/vestra-render/src/blend.rs` owns CPU `source_over`, `blend_pixel`, and
`blend_surface`. `source_over` is used by solid fills and dynamic transformed
image rasterization in `cpu/raster.rs`; `blend_surface` is used by explicit
layer and cached-surface composition in `cpu/compositor.rs`. Normal mode now
selects the direct `source_over` loop once per surface. Add, Screen, Multiply,
and Overlay remain in the existing generic per-pixel path. The accepted
opaque cached-surface bulk copy and opaque transformed-sample shortcut remain
unchanged.

## Profiler interpretation

`vestra_cpu_profile` counters are aggregate worker CPU duration over the full
render operation. They are not wall time and not inherently per-frame. Values
below normalize aggregate milliseconds by rendered frame count to report
CPU-ms/frame. Surface timers are coarse enclosing-loop timers; no per-pixel
`Instant` was added. Composition case counts are worker-local and emitted only
when `VESTRA_CPU_PROFILE` is enabled.

## Composition profiling

The existing coarse `layer_composition` timer isolates explicit surface
composition. The shared source-over portion inside transformed rasterization
remains included in `source_rasterization` / `transform_sampling`; the direct
normal raster path does not call `blend_surface`. On the 60-frame dedicated
composition workload, matched five-run aggregate layer-composition medians
were `28,244.8 ms` before and `27,493.0 ms` after, or `470.747` and
`458.217 CPU-ms/frame`, a `2.66%` reduction. The timing includes surface
iteration and source-over arithmetic, not just the helper call.

## Composition case distribution

The retained composition-focused profile classified `663,552,000` pixels
across the 60-frame operation:

```text
effective source alpha == 0:  23,082,077 (3.4786%)
effective source alpha == 1: 109,395,611 (16.4864%)
destination alpha == 0:                0 (0.00%)
destination alpha == 255:    531,074,312 (80.0351%)
general partial case:                   0 (0.00%)
```

The displayed percentages are rounded; the counter categories are mutually
exclusive because source-alpha cases are classified before destination-alpha
cases.

## Fresh baselines

All benchmark samples used Cargo release, CPU backend, automatic eight
workers, two in-process warmups, five independent processes, and the existing
Phase 10 NullSink harness. The composition-focused fixture is 1280×720,
60 frames, 12 overlapping dynamic normal layers, partial opacities cycling
through `0.2, 0.35, 0.5, 0.65, 0.8, 1.0`, and camera-shake-driven surfaces.

Pre-edit raw warm-2 samples, total wall milliseconds per frame:

```text
composition-focused: 233.889399, 229.680521, 232.552612, 233.425789, 234.324160
animated transform:  14.783151, 15.176936, 14.994306, 14.794142, 14.934001
mixed 10-layer:      86.759083, 99.573733, 96.599789, 99.152325, 98.338845
long combined:       248.972, 247.354, 249.768, 249.507, 251.205
```

Medians are respectively `232.552612`, `14.934001`, `98.338845`, and
`249.507 ms/frame`. The mixed fixture is accurately named mixed
static/dynamic 10-layer; its warm cache result had 150 static-cache hits.

## Current source_over formula

The pre-change and retained formula treats pixels as straight alpha:

```text
source_alpha      = source.a / 255.0 * opacity
destination_alpha = destination.a / 255.0
alpha             = source_alpha + destination_alpha * (1 - source_alpha)
```

For each RGB channel it computes the alpha-weighted straight-color result,
divides by `alpha`, rounds to nearest with `f64::round`, clamps to `[0,255]`,
and casts to `u8`. Output alpha is `(alpha * 255.0).round().clamp(0,255)`.
The zero-alpha result is transparent black. The retained transparent-source
branch returns the destination when its alpha is nonzero and transparent
black otherwise; this is exactly the former result because source alpha is
zero.

## Candidate optimizations

| Idea | Correctness | Performance | Decision |
| --- | --- | --- | --- |
| Effective source-alpha zero return | Exhaustive alpha/reference test and opacity cases passed byte-for-byte | Included in retained result; useful cases were 3.35% of the focused classified work | Keep |
| Hoist Normal mode selection out of `blend_surface` | Whole-surface reference comparison passed | Composition-focused median `232.553→230.097 ms/frame`; layer timer `28,244.8→27,493.0 ms` aggregate | Keep |
| Opaque destination arithmetic specialization | Not implemented; current straight-alpha ordering is preserved | Not measured independently | Reject for this subphase |
| Transparent destination arithmetic specialization | Not implemented; no occurrences in the primary composition profile | Not measured independently | Reject |
| Raw RGBA surface traversal | Not implemented; current `image` linear iterators are already used | No independent evidence justified a second traversal rewrite | Reject |
| Layer-opacity constant rewrite | Not implemented; opacity is already passed once per surface | No evidence of repeated avoidable layer-level work | Reject |
| New non-normal blend framework/formulas | Would expand scope and risk semantics | Not measured | Reject |

## Selected implementation

Production changes are confined to CPU blend execution and its opt-in
worker-local diagnostics. Normal `blend_surface` selects its exact source-over
loop once. `source_over` skips arithmetic only for effective source alpha zero.
No accepted raster fast path was duplicated or changed.

## Fast paths

Retained cases are:

```text
effective source alpha == 0 → return destination (or transparent black)
normal surface blend        → dedicated source-over loop
opacity == 1 and sample alpha == 255 → existing raster sampled-pixel copy
normal opacity == 1 and fully opaque cached surface → existing bulk copy
```

The last two were pre-existing accepted paths and were not redesigned.

## Arithmetic semantics

Alpha formula, opacity multiplication, RGB formula, operation ordering,
`f64` precision, rounding, clamping, and blend ordering are unchanged. Other
blend modes retain their previous formulas and generic dispatch.

## Byte-equivalence

`source_over_fast_path_is_byte_identical_to_reference` exhausts all 256×256
source/destination alpha pairs across seven RGB tuples and six opacity values,
including zero, near-zero, midpoint, near-one, and one. The reference copies
the former arithmetic verbatim. `normal_surface_is_byte_identical_to_reference_surface`
compares complete 37×29 mixed-alpha surfaces across five opacity values.

## Whole-surface correctness

The complete normal surface comparison passed byte-for-byte. Existing normal,
non-normal, alpha, compositor, worker-count, and CPU/WGPU parity tests also
passed without changing non-normal semantics.

## Raster correctness

Existing dynamic-raster tests covering opaque and non-opaque transformed
samples passed. The shared source-over helper remains used by solid fills and
the dynamic transformed-image fallback; no sampler, crop, transform, or
colour-transform code changed.

## Architecture

All retained changes remain in `vestra-render` CPU blend execution and metrics.
No renderer-independent semantics or WGPU shader/blend state changed.

## Memory behavior

No new full-frame allocation, pixel buffer, persistent alpha cache, or
cross-frame state was introduced. The optional case counts are five worker-
local integers and do not allocate or synchronize in the pixel loop.

## Parallelism

No threads, nested parallelism, worker-policy changes, global locks, atomics,
or scheduling changes were introduced.

## Before benchmark

Composition-focused before raw warm-2 samples were:

```text
233.889399, 229.680521, 232.552612, 233.425789, 234.324160 ms/frame
```

True median: `232.552612 ms/frame`, `4.300 FPS`.

## After benchmark

Matching after raw warm-2 samples were:

```text
211.383498, 230.097124, 230.560988, 229.996980, 231.819001 ms/frame
```

True median: `230.097124 ms/frame`, `4.346 FPS`.

## Performance delta

Composition-focused wall speedup is `1.0107x`, throughput improvement
`1.07%`, and frame-time reduction `1.06%`.
The gain is intentionally reported as modest because the workload is also
dominated by dynamic source rasterization.

## Composition timing delta

For 60 frames, aggregate layer-composition medians changed from
`28,244.841 ms` to `27,492.963 ms`; normalized values changed from
`470.747 CPU-ms/frame` to `458.216 CPU-ms/frame`, a `2.66%` reduction.

## Animated-transform impact

The fresh five-run after warm-2 samples were:

```text
14.749530, 14.435553, 15.486142, 14.971446, 16.839710 ms/frame
```

Median `14.971446 ms/frame`, `66.795 FPS`. Composition counters were zero;
this workload uses the direct raster path, so no composition-kernel gain is
claimed.

## Multi-layer impact

For the mixed static/dynamic 10-layer workload, after warm-2 samples were:

```text
87.859155, 98.654362, 97.776000, 99.200771, 99.412193 ms/frame
```

Median `98.654362 ms/frame`, `10.136 FPS`. Static-cache behavior remained
150 warm hits, zero misses, and the existing retained scratch capacity.

## Long-combined impact

After warm-2 samples were:

```text
243.703077, 245.466350, 246.730789, 243.634364, 245.704285 ms/frame
```

Median `245.466350 ms/frame`, `4.074 FPS`, versus the pre-edit median
`249.507 ms/frame`, `4.008 FPS`. Composition normalized median was about
`115.4 CPU-ms/frame` after the change; source rasterization was about
`296.3 CPU-ms/frame`.

## Kernel vs whole-render impact

The dedicated composition timer reduction is `2.66%`; the dedicated whole
render median reduction is `1.06%`; the long-combined wall median reduction is
`1.62%`. These are separate measurements, not interchangeable claims.

## Regression matrix

| Workload | Result |
| --- | --- |
| simple/static | Existing workspace and renderer tests passed |
| composition-focused | Five-run matched benchmark and byte-reference tests passed |
| animated transform | Five-run after benchmark; existing raster tests passed |
| mixed static/dynamic 10-layer | Five-run after benchmark; cache behavior stable |
| ZoomBlur | Existing zoom-blur exact tests passed; code unchanged |
| Color Adjust | Existing LUT exact tests passed; code unchanged |
| Chromatic Aberration | Existing exact kernel tests passed; code unchanged |
| short mixed | Existing Phase 10 harness/test coverage passed |
| long combined | Five-run after benchmark; existing effect tests passed |

## CPU/WGPU parity

Applicable renderer parity tests passed in `cargo test -p vestra-render
--all-features`, including blend-mode and RGBA parity tests. No WGPU code was
changed.

## Tests

```text
cargo test -p vestra-render --all-features
218 passed, 0 failed, 2 ignored

cargo test -p vestra-render --all-features blend::tests::
5 passed, 0 failed

cargo test -p vestra-render --all-features cpu::compositor::tests::
24 passed, 0 failed
```

## Full validation

Final command:

```text
./scripts/check.sh
exit 0
```

Format, workspace clippy, all-feature workspace tests, schema validation, and
schema comparison completed successfully.

## Public compatibility

Rust public API: unchanged. Python API: unchanged. CLI: unchanged. Project
JSON/schema: unchanged.

## Audit

This document: `docs/audits/cpu-alpha-composition-2026-08-13.md`.

## Diff inspection

The final handoff includes fresh `git diff --stat` and `git status --short`.
The temporary Phase 10 generated-results modification was restored; no
unrelated production change is retained.

## Remaining bottleneck

The fresh long-combined normalized exclusive ranking, using 180 frames and
excluding inclusive parent timers, is approximately:

```text
ZoomBlur                 679.9 CPU-ms/frame
Source rasterization     295.8 CPU-ms/frame
Sharpen Gaussian         239.2 CPU-ms/frame
Bloom Gaussian           182.1 CPU-ms/frame
Chromatic Aberration     125.2 CPU-ms/frame
Layer composition        115.4 CPU-ms/frame
Vignette                  97.7 CPU-ms/frame
Global/post and other work remain below these classes.
```

The non-overlapping displayed counters use the 180-frame denominator. The
next target should be selected from a fresh profile rather than assumed from
this composition result.

## Recommended next subphase

Recommend exactly one next target: real-hardware WGPU benchmarking and hot-path
validation, because CPU composition is now a small share and the remaining
CPU micro-optimization targets have high measurement cost.

## Scope confirmation

Source-raster production code was not otherwise redesigned. ZoomBlur was not
changed. Chromatic Aberration was not changed. Color Adjust was not changed.
Gaussian/Bloom was not changed. Vignette was not optimized. No WGPU work was
included. No worker-policy changes were included. No unrelated
feature/refactor was included.

CPU alpha composition hot-path optimization is ready for validation.
