# CPU source-raster Stage 2 audit

## Summary

No Stage-2 source-raster production optimization was retained. Two exact
experiments were measured and rejected because they regressed the primary
animated-transform workload. The accepted Stage-1 opaque composition fast path
remains unchanged.

This result is deliberately narrow. No ZoomBlur, Chromatic Aberration,
Gaussian, Bloom, WGPU, worker, surface, encoder, public API, or schema work was
included.

## Environment and validation

Measurements used the existing Linux x86_64 host, Cargo release builds, CPU
backend, eight workers, the existing Phase 10 NullSink harness, fixed fixtures,
and the same frame ranges and resolutions for each comparison.

The required pre-edit command was:

```text
./scripts/check.sh
exit 0
```

It completed fmt, clippy, workspace tests, schema validation, and schema
comparison before the experiments.

## Profiler interpretation

The CPU profile counters accumulate worker CPU durations across the complete
render operation. They are not per-frame values. A counter is normalized as:

```text
CPU-ms/frame = aggregate counter in milliseconds / rendered frame count
```

For example, a 100-frame counter of `104.645 ms` is `1.04645 CPU-ms/frame`.
The `transform_sampling` timer starts immediately around
`draw_resolved_image` and therefore includes the dynamic image raster loop,
including inverse mapping, bounds checks, bilinear sampling, colour
transformation, and sample composition. It does not isolate matrix arithmetic.

## Current raster hot path

`draw_resolved_image` obtains the transformed visible bounds, maps the first
pixel centre of each row with the inverse affine, and increments the mapped
coordinate horizontally by the inverse matrix delta. Each covered destination
pixel then maps crop coordinates, calls the canonical premultiplied-alpha
bilinear sampler, applies the colour transform, and uses the Stage-1 exact
opaque-sample composition branch when layer opacity is one and the filtered
sample alpha is 255. All other samples use the existing source-over path.

The sampler still reads four neighbours through `RgbaImage::get_pixel` and
keeps the existing edge rejection, pixel-centre convention, operation order,
and f64 arithmetic.

## Fresh baselines

All values below are warm-2 measurements from five independent processes. The
raw samples are total wall milliseconds per frame, not aggregate profiler
milliseconds.

### Animated transform

100 frames at 1280x720:

```text
14.783151, 15.176936, 14.994306, 14.794142, 14.934001 ms/frame
median 14.934001 ms/frame, range 14.783151-15.176936
median 66.961 FPS
```

The corresponding source-raster profile was about 10.8 seconds aggregate per
100-frame operation, or about 108 CPU-ms/frame after normalization.

### Dynamic multi-layer

The existing `many-layers-10` fixture was used at 1280x720 for 30 frames:

```text
86.759083, 99.573733, 96.599789, 99.152325, 98.338845 ms/frame
median 98.338845 ms/frame, range 86.759083-99.573733
median 10.169 FPS
```

Warm-2 cache metrics were 150 static-cache hits, zero misses, 40 static layer
renders in the cold leg, and no new scratch allocation in the warm leg.

### Long combined

The representative `long-combined` fixture was used at 1920x1080 for 180
frames:

```text
242.172352, 242.439155, 244.475251, 246.448194, 244.479302 ms/frame
median 244.475251 ms/frame, range 242.172352-246.448194
median 4.090 FPS
```

The warm-2 source-raster profile was approximately 53,439 aggregate CPU-ms,
or approximately 296.9 CPU-ms/frame. The value is aggregate worker time
normalized by 180, not wall time.

The fresh exclusive ranking still places ZoomBlur first, source rasterization
next, then Gaussian-derived work, composition, and the remaining effects. The
source-raster bucket remains a worthwhile general target, but the tested
changes did not improve it.

## Candidate optimizations

| Idea | Correctness | Performance | Memory | Decision |
| --- | --- | --- | --- | --- |
| Safe raw RGBA slice bilinear sampler with cached dimensions and row stride | Isolated sampler output was byte-identical across 1x1, small, and normal images, fractional coordinates, edges, and mixed alpha. No whole-raster fast-path comparison was retained. | Animated-transform median changed from `14.934001` to `16.720124 ms/frame` in the five-run experiment. Source-raster samples were about `12.3-12.6 s` aggregate per 100 frames, worse than the baseline. | No allocation, unsafe access, lock, or full-frame storage. | Rejected |
| Hoist crop and source-dimension constants outside the pixel loop | Algebraically unchanged, but no independent whole-image byte-equivalence suite was retained after the performance rejection. | Animated-transform median was `16.778240 ms/frame` in the isolated five-run experiment, with the five samples between `16.616072` and `16.928486`. | No new allocation or persistent state. | Rejected |
| Incremental coordinate stepping beyond the existing horizontal scanline step | Rejected before production use because changing independent affine evaluation to repeated floating-point addition can change floor and neighbour selection. | Not benchmarked. | No added storage proposed. | Rejected |
| Transform-class fast paths, tighter bounds, and identity/translation specializations | No implementation was attempted. The current geometry and crop semantics need a stronger fixture-level reference before adding duplicate paths. | Not measured. | No storage proposed. | Rejected for this subphase |

The raw-slice experiment was removed after the regression. The invariant-hoist
experiment was also removed. The production tree therefore remains at the
accepted Stage-1 implementation.

## Byte-equivalence and semantics

The repository's existing CPU raster, compositor, crop, and CPU/WGPU parity
tests passed after restoring the accepted implementation. No new production
fast path remains to compare against the canonical raster path.

The following semantics remain unchanged:

```text
pixel-centre convention
inverse-transform convention
bilinear interpolation and f64 precision
edge rejection and clamping behavior
crop mapping and prepared crop caching
alpha and opacity handling
colour-transform order
destination clipping
```

## Architecture and resource behavior

No code was retained outside the existing CPU renderer. The experiments did
not change the compiler, core transform semantics, assets cache, worker state,
EffectSurfacePool, WGPU backend, or public interfaces.

No new threads, nested parallelism, global locks, full-frame surfaces,
per-pixel allocations, coordinate tables, or cross-frame state were added.
Static caching, crop caching, random-access rendering, and opt-in profiling
remain on the accepted paths.

## Before and experimental after benchmarks

The before samples are the five repeated sets above. The raw-sampler
experimental after animated-transform samples were:

```text
16.637942, 16.988603, 16.954191, 16.720124, 16.605285 ms/frame
median 16.720124 ms/frame, median 59.808 FPS
```

The isolated hoist experiment samples were:

```text
16.897586, 16.721802, 16.616072, 16.928486, 16.778240 ms/frame
median 16.778240 ms/frame, median 59.601 FPS
```

These are experimental results only. They are not final production results,
because both changes were removed.

## Performance delta

Relative to the animated baseline, the raw-sampler experiment was about 11.95%
slower by median frame time. The hoist experiment was about 12.35% slower.
Neither candidate reached the material-improvement threshold.

No source-raster timing delta, multi-layer after delta, or long-combined after
delta is claimed for production because no change survived measurement.

## Regression matrix

The focused and canonical suites covered:

```text
simple/static cache behavior: passed
animated transform: measured; experiments rejected
dynamic multi-layer: measured baseline; no production change retained
ZoomBlur: existing CPU tests passed, production unchanged
Color Adjust LUT: existing CPU tests passed, production unchanged
Chromatic Aberration: existing byte-equivalence tests passed, production unchanged
short mixed and long combined: existing render coverage passed
Gaussian/Bloom: existing byte-equivalence and effect tests passed
```

## Tests

The targeted validation completed successfully:

```text
cargo fmt --check
exit 0

cargo test -p vestra-render --all-features
218 passed, 0 failed, 2 ignored

cargo test -p vestra --all-features
82 unit tests passed; 2 public-export tests passed; 34 public-SDK tests passed
```

The final canonical validation result is recorded below after the final run.

## Compatibility and scope

Because the production experiments were removed, these interfaces are
unchanged:

```text
Rust API: unchanged
Python API: unchanged
CLI: unchanged
project JSON/schema: unchanged
```

The only retained change in this subphase is the corrected Chromatic audit
accounting. No source-raster production code changed.

## Remaining bottleneck and next target

The tested source-raster candidates do not justify a Stage-2 implementation.
The next target should be selected from a fresh normalized profile rather than
preselected here. ZoomBlur remains out of scope for this subphase and is not
scheduled by this audit.

## Final canonical validation

```text
./scripts/check.sh
exit 0
```
