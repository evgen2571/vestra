# CPU Gaussian convolution subphase

## Summary

This subphase first isolated the mixed CPU workload's effect costs, then
optimized one kernel: the CPU Gaussian convolution used by Glow/Bloom and by
the standalone Gaussian and Sharpen pass plans. No other effect algorithm was
changed.

## Environment and validation

Measurements ran on Linux x86_64, Intel Core i7-8750H, Rust 1.96.1, FFmpeg
7.1.5, Cargo release profile, CPU backend, and the automatic eight-worker
policy. The pre-edit `./scripts/check.sh` exited 0. The final command was run
again after the implementation.

## Mixed fixture

The representative fixture is `examples/projects/effects-ready-v1.json`,
rendered at 1920x1080 for 30 frames with the preview quality setting. It has
two image clips. The outgoing clip has Glow, the incoming clip has Chromatic
Aberration, and the global post-effect chain has Color Adjust, Vignette, and
Sharpen. The important parameters are Glow threshold 0.6, radius 4, intensity
0.35, and pink tint; Chromatic Aberration amount 2; Color Adjust exposure 0.1;
Vignette amount 0.18, radius 0.72, softness 0.35; and Sharpen amount 0.25,
radius 1.0.

## Profiling model and isolation

The existing CPU profile records aggregate worker CPU time. It is exclusive at
the effect timing level: each effect is timed around its own pass sequence.
`global_post_effect` is an inclusive parent around the global effect chain and
must not be added to the child effect timings. `effect_execution` is the sum of
the effect timings. Source transform sampling is a subset of source
rasterization.

The profiler was refined only enough to split the active `other_effects`
implementations into Chromatic Aberration, Vignette, Color Adjust, and
Sharpen. Glow/Bloom was also split into Highlight Extract, its two Gaussian
passes, and Additive Composite. Timing remains opt-in through
`VESTRA_CPU_PROFILE`; normal rendering does not create these timers.

The representative isolated warm-2 profile measured these exclusive mixed
costs in aggregate CPU-ms/frame:

| Kernel or effect | CPU-ms/frame |
| --- | ---: |
| Glow/Bloom total | 440.49 |
| Glow/Bloom Gaussian pair | 298.95 |
| Sharpen | 272.22 |
| Color Adjust | 202.44 |
| Glow/Bloom additive composite | 94.25 |
| Vignette | 94.04 |
| Glow/Bloom highlight extract | 47.29 |
| Chromatic Aberration | 0 |

The Gaussian pair was the largest concrete inner kernel and the largest
measured effect contribution in the mixed fixture. That is why this subphase
selected the Gaussian convolution loop. The parent global-post value was not
treated as another kernel.

## Benchmarks

The repeated mixed baseline used:

```bash
VESTRA_PHASE10_BENCH=1 VESTRA_CPU_PROFILE=1 \
VESTRA_PHASE10_BENCH_START=13 VESTRA_PHASE10_BENCH_LIMIT=1 \
VESTRA_PHASE10_BENCH_OUTPUT=/tmp/vestra-mixed-isolated-baseline-N.json \
cargo test --release -p vestra phase10_release_matrix --all-features -- --nocapture
```

After adding the focused matrix row, the mixed row is index 14. The matched
after command used the same settings and output path prefix
`/tmp/vestra-mixed-after-N.json`.

The five mixed warm-2 wall samples before were 187.888, 186.289, 188.967,
186.294, and 190.242 ms/frame. Median was 187.888 ms/frame, or 5.322 FPS.
After samples were 187.287, 188.166, 187.652, 187.661, and 187.917 ms/frame.
Median was 187.661 ms/frame, or 5.329 FPS.

The focused workload disables the static cache so the Gaussian kernel runs on
all 100 frames at 1280x720 with eight workers. It uses two in-process warm
operations in the existing harness and five independent measured processes.

| Set | Samples, ms/frame | Median ms/frame | Median FPS |
| --- | --- | ---: | ---: |
| Before | 30.988, 31.988, 32.402, 32.052, 31.713 | 31.988 | 31.261 |
| After | 32.090, 31.572, 32.105, 31.675, 31.860 | 31.860 | 31.387 |

The focused wall-time speedup is 1.004x. Throughput improves 0.40% and frame
time falls 0.40%. The Gaussian CPU timing changed from a median 97.439 to
95.904 CPU-ms/frame, a 1.58% kernel reduction. These kernel and whole-render
percentages are different because source rasterization and composition remain
outside the selected kernel.

## Implementation

The old loop accessed source pixels and wrote target pixels through per-pixel
image methods, recomputing coordinate-to-byte work on every sample. The new
loop keeps the same offset order, weight order, f64 arithmetic, clamping, and
premultiplied-alpha accumulation, but traverses the existing RGBA backing
slices directly. It caches the row stride and current source row, computes one
byte offset per sample, and writes one four-byte output slice.

No sample count, radius, quality setting, interpolation mode, pass count, or
effect ordering changed. No new full-frame surface, heap cache, per-row
allocation, or per-sample allocation was introduced. The only extra storage is
the existing bounded Gaussian weight vector and scalar locals.

## Correctness and regression matrix

`optimized_gaussian_convolution_is_byte_identical_to_reference` compares the
optimized loop with a test-only copy of the former loop for horizontal and
vertical passes, radii 1.0, 2.5, and 4.0, and a 9x7 RGBA image containing opaque,
semi-transparent, and transparent pixels. Existing Gaussian, Glow, Sharpen,
ZoomBlur, alpha, WGPU parity, and random-access tests also passed.

Post-change release regression rows used the existing phase10 harness:

| Workload | Resolution | Frames | Warm-2 ms/frame |
| --- | ---: | ---: | ---: |
| Pipeline floor | 1280x720 | 30 | 0.201 |
| Static image | 1280x720 | 300 | 0.149 |
| Focused Gaussian | 1280x720 | 100 | 27.377 |
| Animated transform | 1280x720 | 100 | 15.505 |
| ZoomBlur transition | 1280x720 | 100 | 99.157 |
| Mixed workload | 1920x1080 | 30 | 183.991 |

The ZoomBlur path remains separate and unchanged. Random-access rendering,
worker behavior, surface reuse, and WGPU code were not changed.

## Architecture and compatibility

The optimization remains in `vestra-render`'s CPU effect implementation.
Renderer-independent effect semantics and the WGPU implementation are
unchanged. There are no Rust public API, Python API, CLI, or project
JSON/schema changes. No threads, nested parallelism, locks, unsafe code, or
worker-policy changes were added.

## Remaining bottleneck and next subphase

Fresh mixed profiles still rank the selected Glow/Bloom Gaussian pair first,
followed by Sharpen, Color Adjust, Vignette, and Glow/Bloom composite. The
whole mixed wall time remains dominated by the combined source, effect, and
composition workload. The next single performance target should be the
measured Sharpen kernel, after a new focused baseline confirms its value.

## Scope

Only one measured CPU kernel was optimized. No quality reduction, unrelated
Gaussian/Bloom/other-kernel rewrite, WGPU work, worker-policy work, public API
change, or unrelated refactor was included.
