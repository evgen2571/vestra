# CPU Sharpen isolation and Color Adjust subphase

## Summary

This subphase refined the opt-in CPU profiler to separate Sharpen's inclusive
effect timing into its two Gaussian passes and exclusive Unsharp composite.
Fresh measurements showed that Unsharp was not the next non-Gaussian target:
Color Adjust was approximately five times more expensive in the representative
mixed workload. The one selected kernel was CPU Color Adjust.

The retained implementation hoists the frame-invariant exposure scale and
gamma exponent out of the per-pixel loop. A larger raw-RGBA traversal attempt
was measured and removed because it regressed focused wall time.

## Environment and validation

Measurements used Linux x86_64, the existing Intel Core i7-8750H host, Rust
1.96.1, FFmpeg 7.1.5, the Cargo release profile, CPU backend, and automatic
8-worker policy. The required pre-edit command passed with exit 0:

```text
./scripts/check.sh
```

The first final-check attempt reached source compilation but failed because
the filesystem was full. Removing only generated Cargo `target/` artifacts
freed 9.9 GiB; rerunning the exact command passed with exit 0.

## Fixtures and method

The mixed fixture was `examples/projects/effects-ready-v1.json`, rendered at
1920x1080 for 30 frames with preview quality, automatic 8 workers, and the
existing NullSink Phase 10 harness. It contains Glow, Chromatic Aberration,
Color Adjust, Vignette, and Sharpen. Important authored/evaluated values are
Glow threshold 0.6/radius 4/intensity 0.35, Chromatic Aberration amount 2,
Color Adjust exposure 0.1, Vignette amount 0.18/radius 0.72/softness 0.35,
and Sharpen amount 0.25/radius 1.0.

The focused selected-kernel fixture is `examples/effects/color-adjust.json`,
rewritten by the existing harness to 1280x720 and 100 frames. It uses the
fixture's animated Color Adjust parameters and the same automatic 8 workers.
Each benchmark set used five independent release processes, with the
harness's two in-process warm operations; `warm-2` is reported.

Mixed baseline command:

```bash
VESTRA_PHASE10_BENCH=1 VESTRA_CPU_PROFILE=1 \
VESTRA_PHASE10_BENCH_START=14 VESTRA_PHASE10_BENCH_LIMIT=1 \
VESTRA_PHASE10_BENCH_OUTPUT=/tmp/vestra-sharpen-isolation-baseline-N.json \
cargo test --release -p vestra phase10_release_matrix --all-features -- --nocapture
```

Focused Color Adjust command used matrix row 15 with the same command shape.
The focused row was added to the existing Phase 10 harness, not a new
benchmark system.

## Sharpen decomposition

The refined profiler reports aggregate worker CPU time per frame. `sharpen` is
the inclusive parent timing around all three Sharpen passes. The two new
counters are exclusive pass timings: `sharpen_gaussian` is the sum of the
horizontal and vertical Gaussian pass timers, and
`sharpen_unsharp_composite` is the Unsharp composite timer. They must not be
added together with `sharpen`.

Fresh mixed warm-2 medians:

| Timing | Aggregate CPU-ms/frame |
| --- | ---: |
| Sharpen total (inclusive) | 268.16 |
| Sharpen Gaussian H/V (exclusive child sum) | 227.63 |
| Sharpen Unsharp composite (exclusive child) | 41.05 |

The parent/children reconcile within timer overhead. `global_post_effect` is
also an inclusive parent around the global post-effect chain; it is not added
to child effects. `effect_execution` is the aggregate sum of effect parents.

## Remaining concrete-kernel ranking

The following fresh mixed warm-2 medians are exclusive concrete timings unless
explicitly marked as a parent:

| Kernel | CPU-ms/frame | Share of listed concrete effect CPU |
| --- | ---: | ---: |
| Glow/Bloom Gaussian pair | 306.23 | 31.4% |
| Sharpen Gaussian H/V | 227.63 | 23.3% |
| Color Adjust | 206.24 | 21.1% |
| Vignette | 94.85 | 9.7% |
| Bloom additive composite | 96.17 | 9.9% |
| Sharpen Unsharp composite | 41.05 | 4.2% |
| Bloom highlight extraction | 47.71 | 4.9% |
| Chromatic Aberration | 0.00 | 0.0% |

The percentages are normalized over the listed concrete rows and are only a
ranking aid; Gaussian-derived work is listed separately from non-Gaussian
selection. Glow/Bloom total was approximately 451.12 CPU-ms/frame, and the
inclusive global-post parent was approximately 576.45 CPU-ms/frame.

## Selection rationale

Color Adjust was selected because it was the largest remaining concrete
non-Gaussian kernel at approximately 206 CPU-ms/frame, substantially above
Vignette, Bloom composite, Bloom extraction, and Unsharp. It is a general
per-pixel kernel and its invariant factors offer a simple exact optimization.
Sharpen Gaussian work was deliberately not selected, and no generic Gaussian
implementation was changed.

## Pre-change algorithm and implementation

Before the change, each Color Adjust pixel/channel evaluated the same
frame-invariant `2^exposure` and `1/gamma.max(0.001)` values three times. The
loop also retained the existing encoded-byte arithmetic, clamp, `powf`, round,
and alpha preservation.

The production loop now computes these two invariant values once per pass and
reuses them for all RGB channels. Existing `RgbaImage::enumerate_pixels` and
`put_pixel` traversal remains in place. An attempted safe raw-slice/chunk
traversal was benchmarked, produced byte-identical pixels, but regressed the
focused wall median from 25.65 to 26.99 ms/frame; it was removed.

No authored parameters, operation ordering, numeric types, quality settings,
or effect ordering changed.

## Quality and pixel correctness

`optimized_loop_is_byte_identical_to_the_former_loop` compares the retained
implementation to a test-only copy of the former loop over a 7x5 RGBA image
with opaque, semi-transparent, and transparent pixels, mixed channels, dark
and bright values, and identity, weak, and strong parameter combinations.
The existing Color Adjust golden test also passes. Output is byte-identical
to the former implementation for those cases.

## Architecture, memory, and parallelism

The change remains in `crates/vestra-render/src/cpu/colour_adjust.rs`, inside
the existing CPU renderer effect execution path. Renderer-neutral pass plans,
WGPU shaders, and public layers are unchanged.

There are no new full-frame allocations, temporary images, per-row or
per-pixel allocations, caches, or surfaces. The same existing source and
target effect surfaces are used. No threads, nested parallelism, global lock,
unsafe code, worker-policy change, or profiler-loop overhead was introduced.
Profiling remains opt-in and the enablement decision is still resolved once
when the CPU backend is built.

## Focused performance results

### Before

| Samples, ms/frame | Median | Range | Median FPS |
| --- | ---: | ---: | ---: |
| 23.221, 25.503, 25.651, 25.661, 26.498 | 25.651 | 23.221–26.498 | 38.986 |

Color Adjust aggregate CPU samples were 38.092, 43.359, 42.924, 43.281,
and 43.344 CPU-ms/frame; median 43.281.

### After

| Samples, ms/frame | Median | Range | Median FPS |
| --- | ---: | ---: | ---: |
| 25.236, 25.401, 25.893, 26.229, 26.276 | 25.893 | 25.236–26.276 | 38.620 |

Color Adjust aggregate CPU samples were 42.832, 43.218, 42.806, 43.033,
and 42.932 CPU-ms/frame; median 42.932.

The selected kernel reduction is 0.81%. Focused wall time changed by -0.94%
in the frame-time formula (the after median was slower), so no whole-render
speedup is claimed. The change is retained because it is a minimal,
obviously maintainable exact invariant hoist; a larger traversal rewrite was
rejected.

## Mixed workload impact

Fresh mixed warm-2 wall samples before were 186.316, 184.782, 186.591,
186.783, and 186.234 ms/frame. Median: 186.316 ms/frame, range
184.782–186.783, 5.367 FPS.

After samples were 206.488, 187.007, 188.226, 190.089, and 195.871
ms/frame. Median: 190.089 ms/frame, range 187.007–206.488, 5.261 FPS.

The measured median mixed frame time was 2.02% higher; this noisy result is
reported transparently and is not attributed as a regression to the 0.8%
kernel change. Kernel CPU improvement and whole-render wall impact remain
separate measurements.

## Regression matrix

Fresh post-change Phase 10 warm-2 rows included:

| Workload | Resolution | Frames | Workers | ms/frame | FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Pipeline floor | 1280x720 | 30 | 8 | 0.186 | 5374.20 |
| Animated transform | 1280x720 | 100 | 8 | 15.062 | 66.39 |
| ZoomBlur transition | 1280x720 | 100 | 8 | 99.743 | 10.03 |
| Global post (cache-warm) | 1280x720 | 30 | 8 | 0.187 | 5347.31 |
| Focused Color Adjust | 1280x720 | 100 | 8 | 25.893 | 38.62 |
| Mixed realistic project | 1920x1080 | 30 | 8 | 190.089 | 5.26 |

The retained ZoomBlur result remains near the accepted ~98.806 ms/frame /
10.121 FPS result, and the retained animated-transform result remains near
the accepted ~16.739 ms/frame / 59.742 FPS result. These rows are regression
evidence, not claims of improvement in this subphase.

## Verification

Passed targeted commands:

```text
cargo test -p vestra-render                         # 213 passed, 2 ignored
cargo test -p vestra                                # 82 + 2 + 34 tests passed
cargo test -p vestra-render colour_adjust           # 2 passed
```

The final canonical command passed with exit 0:

```text
./scripts/check.sh
```

It completed fmt check, workspace clippy with warnings denied, all workspace
tests, Python schema validation, and generated-schema comparison.

## Compatibility and scope

Rust public API, Python API, CLI behavior, project JSON/schema, WGPU code,
worker architecture, and worker policy are unchanged. Only one measured
non-Gaussian kernel was optimized: CPU Color Adjust. Sharpen instrumentation
is internal and opt-in. No Gaussian micro-optimization, quality reduction,
new allocation, threading change, or unrelated refactor was included.

## Remaining bottleneck and recommendation

Fresh post-change mixed concrete costs remain dominated by Glow/Bloom Gaussian
pair (~306.23 CPU-ms/frame), Sharpen Gaussian H/V (~227.63), Color Adjust
(~206.24), Vignette (~94.85), Bloom additive composite (~96.17), Bloom
highlight extraction (~47.71), and Sharpen Unsharp (~41.05). The next single
subphase should be an algorithm-level Gaussian/Bloom strategy investigation,
with explicit quality/parity requirements; another Gaussian indexing tweak is
not recommended.
