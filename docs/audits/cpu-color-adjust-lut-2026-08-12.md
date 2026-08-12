# CPU Color Adjust exact lookup-table subphase

## Summary

This subphase replaces repeated nonlinear RGB-channel evaluation in the CPU
Color Adjust pass with one exact 256-entry lookup table per evaluated pass.
The table contains the existing `f64` formula's rounded `u8` result for every
possible input channel value. Alpha and the existing image traversal are
unchanged.

No Gaussian, Bloom, Sharpen, WGPU, worker, surface-flow, encoder, public API,
or schema work is included.

## Environment and validation

Measurements ran on the existing Linux x86_64 host, Intel Core i7-8750H,
Rust 1.96.1, FFmpeg 7.1.5, Cargo release profile, CPU backend, and automatic
8-worker policy. The pre-edit `./scripts/check.sh` completed with exit 0.

The final canonical check was run again after implementation and is recorded
below.

## Workloads and method

The existing Phase 10 release matrix was used. Both focused and mixed runs
used the same machine, CPU backend, release profile, automatic 8 workers,
preview quality, frame ranges, fixtures, and two in-process warm operations.
Each before and after set contains five independent measured processes; the
reported values are the `warm-2` samples.

Focused Color Adjust:

```text
fixture: examples/effects/color-adjust.json
resolution: 1280x720
frames: 100
parameters: animated exposure, gamma, black_point, and white_point tracks
warmups: 2 (cold and warm measurements precede warm-2)
measured runs: 5
```

The representative mixed fixture is
`examples/projects/effects-ready-v1.json` at 1920x1080 for 30 frames with the
same worker and warmup policy. Its global post-effect context is reported
separately because its aggregate `color_adjust` counter did not isolate the
same focused-kernel reduction.

## Pre-change algorithm

For every processed pixel, the old CPU pass evaluated the same frame-invariant
`2^exposure` and `1/gamma.max(0.001)` values once per RGB channel. It then
performed the normalization, exposure multiplication, black-point subtraction,
white-point scaling, clamp, `powf`, multiplication by 255, and round for each
of the three channels. Alpha was copied from the source pixel unchanged.

The focused evaluated parameters vary by frame in the fixture, but are
constant during each individual evaluated pass.

## LUT design and implementation

The implementation is in
`crates/vestra-render/src/cpu/colour_adjust.rs`:

- entries: exactly 256, indexed by input `u8` from `0..=255`;
- storage: local `[u8; 256]`;
- lifetime: one `apply` invocation, corresponding to one evaluated pass;
- construction: once before pixel traversal, never per pixel or per row;
- formula: the existing operation order and `f64` formula, centralized in the
  private `adjust_channel` helper;
- traversal: existing `enumerate_pixels` and `put_pixel` traversal;
- lookup: RGB channels index the table directly; alpha is not indexed.

The table is rebuilt from the current evaluated exposure, gamma, black, and
white values on every invocation. No cross-frame cache, synchronization, or
parameter quantization was introduced.

## Quality and compatibility

Exposure scale, gamma exponent, black/white mapping, input normalization,
clamping, rounding, operation order, and alpha preservation are unchanged.
The optimization is exact over the complete byte input domain; it is not an
interpolated or reduced-precision approximation.

The change is local to the CPU renderer implementation. Renderer-independent
semantics, the WGPU shader and execution path, Rust public APIs, Python APIs,
CLI contracts, and project JSON/schema are unchanged.

## Exhaustive correctness

The new unit test compares all `0..=255` LUT entries with the former formula
for five valid parameter sets covering identity, positive and negative
exposure, gamma below and above one, black/white adjustment, and strong
combined adjustment. The test passes for all 256 values in every set.

The whole-image byte-identical test compares the optimized pass with a
test-only copy of the former implementation on a 7x5 RGBA image containing
zero, near-black, midtone, near-white, and 255 channel values, mixed RGB
channels, and transparent, partial, and opaque alpha. It covers nine valid
parameter combinations and passes byte-for-byte.

Because `apply` constructs the local table from its arguments on every call,
the animated focused fixture also exercises changing evaluated parameters
without carrying a table between frames. Random-access rendering remains
parameter-local and does not depend on prior frames.

## Focused benchmark results

### Before

Warm-2 wall samples were:

```text
26.01044196, 26.16977054, 25.87073071, 26.31893943, 25.98987403 ms/frame
```

Median: 26.01044196 ms/frame; range 25.87073071–26.31893943; median FPS
38.4461.

Color Adjust aggregate CPU samples were 43.11817769, 42.94631672,
42.56597276, 43.74113414, and 43.45466626 CPU-ms/frame. Median:
43.11817769 CPU-ms/frame.

### After

Warm-2 wall samples were:

```text
20.46978003, 20.21278417, 20.26398295, 19.86898277, 20.16082703 ms/frame
```

Median: 20.21278417 ms/frame; range 19.86898277–20.46978003; median FPS
49.4736.

Color Adjust aggregate CPU samples were 2.49108746, 2.47578411, 2.51278134,
2.50757917, and 2.47414009 CPU-ms/frame. Median: 2.49108746 CPU-ms/frame.

The focused wall-time speedup is 1.28683x, throughput improvement is 28.6831%,
and frame-time reduction is 22.2897%. The Color Adjust kernel reduction is
94.2227%. These are distinct kernel and whole-render measurements.

## Mixed workload impact

Mixed warm-2 wall samples before were 190.17206717, 186.49814953,
187.09188680, 188.36032850, and 187.51566893 ms/frame. Median:
187.51566893 ms/frame; median FPS 5.33289.

Matched after samples were 153.21314190, 159.42219667, 160.99436147,
159.15239443, and 195.87149023 ms/frame. Median: 159.42219667 ms/frame;
median FPS 6.27265.

The matched wall medians are 1.17622x, or 17.6221% higher throughput and
14.9819% lower frame time. The samples have substantial run-to-run variance,
including the 195.871 ms after sample and 190.172 ms before sample, so this
whole-render change is not attributed solely to the LUT.

The mixed `color_adjust` aggregate CPU counter was approximately 208.4
CPU-ms/frame before and approximately 208.4 CPU-ms/frame after. This does not
serve as focused-kernel evidence; the focused workload is the authoritative
measurement of the optimized `EffectOperation::ColorAdjust` pass.

## Regression matrix

Post-change release rows used automatic 8 workers and the existing Phase 10
harness:

| Workload | Resolution | Frames | Warm-2 ms/frame | FPS |
| --- | ---: | ---: | ---: | ---: |
| Pipeline floor | 1280x720 | 30 | 0.178 | 5617.20 |
| Static image | 1280x720 | 300 | 0.161 | 6209.72 |
| Animated transform | 1280x720 | 100 | 14.320 | 69.83 |
| ZoomBlur transition | 1280x720 | 100 | 99.743 | 10.03 |

The accepted ZoomBlur result remains approximately 98.806 ms/frame / 10.121
FPS, and the accepted animated-transform result remains approximately 16.739
ms/frame / 59.742 FPS. The current rows show no material regression from this
CPU Color Adjust-only change; they are regression evidence, not claims of
improvement to those unrelated kernels.

## Memory and parallelism

The additional temporary storage is exactly 256 bytes for the local table plus
existing scalar locals. There are no heap allocations, full-frame allocations,
per-row allocations, persistent caches, locks, new threads, worker changes, or
profiling changes. The table is local and immutable after construction.

## Verification commands

Passed:

```text
./scripts/check.sh                         # pre-edit, exit 0
cargo test -p vestra-render colour_adjust  # 3 passed
cargo test -p vestra-render                # 214 passed, 2 ignored
cargo test -p vestra                       # full suite passed
```

The final canonical check completed with exit 0 after this audit was added.

## Remaining bottleneck and next subphase

The fresh mixed profile still shows Gaussian-derived Glow/Bloom and Sharpen
Gaussian work as dominant concrete CPU costs, alongside source rasterization,
composition, and global post work. The next single performance subphase should
be **Algorithm-Level Gaussian/Bloom Optimization**, with its own parity and
quality plan. It is not implemented here.

## Scope

Only the CPU Color Adjust pass received an optimization. No approximate LUT,
quality reduction, Gaussian/Bloom algorithm change, WGPU change, worker-policy
change, public API change, schema change, or unrelated refactor was included.
