# CPU efficiency and work elimination

This pass keeps five measured changes in `vestra-render`. Public models, image
formats, pixel rounding, effect order, and renderer ownership remain unchanged.
Measurements used release builds on an AMD Ryzen 5 3600 under WSL, with 12 logical
CPUs. These are CPU results, not GPU results.

## Retained changes

| Change | Paired measurement | Result |
| --- | --- | --- |
| Return an opaque source directly for Normal composition at opacity exactly one | Canonical combined scene, 720p, five samples per variant | Median 7,233 to 6,900 ms |
| Compose an unchanged full-size group surface without bilinear resampling | Nested groups, 720p, five samples per variant | Median 3,087 to 2,513 ms |
| Precompute the 256 byte-to-unit channel values used by Gaussian convolution | Gaussian-small, 720p, five samples per variant, repeated in reverse order | Repeat median 2,612 to 2,513 ms |
| Allocate one automatic CPU worker for a fully static visual plan | Six prepared static workloads, three paired runs, including cache-disabled rendering | Retained RGBA scratch at 720p falls from 88,473,600 to 11,059,200 bytes |
| Restrict Gaussian pixel work to nonzero-alpha bounds plus finite kernel support | Existing Spectrum2D neon-circle glow/bloom workload, 24 frames at 1080p, eight workers, three paired runs | Median 1,479.887 to 638.940 ms |

Each comparison uses the preceding retained implementation as its reference;
these percentages must not be added together. The isolated source-over comparison
uses the original implementation. The original revision was `8df8bf8`.

The identity surface shortcut requires an exactly identity resolved inverse,
matching dimensions, and no motion tiling. Nearby transforms still resample.
Opaque composition similarly requires exact opacity one, not an epsilon test.

The Gaussian table preserves channel normalization and accumulation order.
Generated assembly retains packed floating-point multiplication/addition and
removes per-tap division. No manual SIMD instructions or unsafe code were added.

The SDK renders a static visual plan once and reuses its pixels. One worker avoids
seven unused worker pools in the measured 720p/1080p cases. Dynamic plans retain
the existing CPU and frame-memory limits. This also changes the capacity returned
by automatic `CpuBackend` construction for direct callers; capacity is discovered
through the backend contract. A client submitting duplicate static frames directly
can trade parallel throughput for lower retained memory. Scratch figures above
are the renderer's RGBA scratch metric, not total process memory.

Gaussian bounds change the amount of convolution work, not scratch dimensions.
The kernel still samples and clamps in original canvas coordinates. Transparent
pixels cannot contribute premultiplied color, so work outside the alpha bounds
expanded by the pass's support produces transparent black. Reused targets are
cleared when the selected region is smaller than the canvas. Images with a
nontransparent corner bypass the bounds scan.

## Bounds cost and limits

The focused benchmark runs two prepared convolution passes, one warmup and five
samples, repeated three times at 1280x720. Allocations occur before timing.

| Input | Radius | Full-frame median ms | Bounded median ms |
| --- | ---: | ---: | ---: |
| Opaque | 1 | 47.431 | 47.650 |
| Opaque | 8 | 110.576 | 111.385 |
| Near-full with transparent border | 1 | 47.211 | 47.648 |
| Near-full with transparent border | 8 | 111.968 | 111.177 |
| Two small, widely separated islands | 1 | 32.510 | 33.140 |
| Two small, widely separated islands | 8 | 94.106 | 95.884 |
| Centered sparse rectangle | 1 | 34.338 | 6.308 |
| Centered sparse rectangle | 8 | 96.896 | 14.330 |

The islands deliberately produce an almost full-frame bounding rectangle. Their
roughly 2% penalty is the retained tradeoff for scanning without much saved work.
The separate dense Gaussian SDK pair measured 2,527 versus 2,532 ms. There is no
claim that every image becomes faster.

The sparse glow/bloom raw times were 1458.314, 1479.887, 1501.038 ms before and
636.050, 645.173, 638.940 ms after. This is a renderer-only workload; it excludes
FFmpeg encoding and is not comparable to a one-shot render's wall time.

## Other requested areas

Existing mechanisms were exercised instead of introducing overlapping caches or
changing core semantics. Functional and resource checks establish work elimination
or reuse; they do not establish an unmeasured speedup.

| Area | Validation and disposition |
| --- | --- |
| No-op elimination and constant folding | Core normalization tests reduce nine authored effects to three compiled effects, normalize two constant tracks, and retain near-identity values. CPU identity-pass and exact zero-effect pixel tests exercise execution. |
| Invisible and off-screen culling | Existing visible-layer checks and bounded raster loops remain. Off-screen particle and rotated scanline tests exercise clipping. Gaussian bounds also eliminate work on fully transparent intermediates. Focused image/group/matte measurements below quantify remaining off-screen work. A broader opaque-cover experiment was rejected below. |
| Transform, opacity and color collapsing | Identity surface sampling is retained. Existing compiler fusion, ordered color, group color and group opacity tests preserve the operation order and single application of group opacity. |
| Static precomputation | Channel normalization is retained; compiler-normalized tracks and color matrices remain the shared semantic mechanism. |
| Static layer, group and mask caching | Static cache tests check one population across 100 frames, activity gaps, dynamic-reference pixels, budgets and groups. Core tests classify static and animated masks. Canonical masks/mattes and prepared static workloads exercise actual renders. |
| Allocation and scratch reuse | Fixed effect and depth-indexed composition pools remain. The dynamic scratch test reaches a stable three-buffer set across 100 frames. Static worker allocation improves retained memory; buffer ownership swaps were rejected. |
| Intermediate bounds | Sparse Gaussian work is bounded without changing full-frame scratch ownership. Prepared source bounds and raster clipping remain in place. |
| Mask and matte fast paths | Static complete-layer and whole-visual reuse avoid rerunning their masks. Existing within-composition matte reuse remains. No additional independent mask cache was introduced. |
| Resized assets, text, shapes, kernels and animation evaluation | Decoded/prepared pixels are shared; transformed or resized output is reused through static complete-layer/visual caching. Dynamic resized images still resample each frame; crop tests check cache bytes and fallback. Text/shape preparation tests and bounded kernel-cache tests pass through the same renderer. Prepared random access exercises static visual reuse. Constant-track normalization removes repeated constant evaluation; repeated and advancing evaluation were measured below, and no dynamic evaluation cache was added. |
| Copies and conversions | Identity group surfaces avoid resampling. Seven worker-plan clones and unused pool allocations disappear for typical static plans. Output frame ownership and required full-frame copies remain intact. |
| SIMD | Assembly and byte-reference tests support the normalization-table change. Manual intrinsics were unnecessary for the measured gain. |
| Parallelism | Existing 1080p worker scaling measured 7213, 3941, 2025 and 1235 ms for one, two, four and eight workers. Retained scratch rises with worker count. Dynamic parallelism remains; static plans use one worker. The separate video scaling test uses a mock decoder and does not measure FFmpeg scaling. |
| Hot image processing | Opaque composition, identity resampling and Gaussian normalization/bounds have paired measurements. Glow/bloom benefits were measured with Spectrum2D. No universal blur, mask or resampling speedup is claimed. |
| Compatible operation fusion | Existing compiled color fusion and direct raster color/composite execution are retained and tested. No fusion across intermediate rounding or retained-original effect boundaries was introduced. |

## Focused cache, culling and evaluation checks

The manual renderer benchmark prepares one worker, warms up once, then measures
five samples of 30 serial frame submissions. These are controlled renderer
experiments, not frame-parallel throughput measurements. Disabling root static
content classification provides the cache-bypass reference; it is not an authored
animation. Cached and uncached output bytes must match.

| Workload | Variant | Median ms per 30 frames |
| --- | --- | ---: |
| Feathered masks, 320x180 | Cached / cache bypass | 17.382 / 519.187 |
| Hidden ellipse matte, 320x180 | Cached / cache bypass | 7.839 / 157.797 |
| Image with blur, 1280x720 | Centered / off-screen | 247.081 / 150.768 |
| Group containing that image | Centered / off-screen | 792.664 / 358.787 |
| Hidden group used as an alpha matte | Centered / off-screen | 961.381 / 544.390 |

Both cached cases record 30 cache hits and zero new static-layer renders per
sample. Separate profiling runs show no source rasterization in those cache-hit
samples. The image's off-screen rasterization falls from 26.058 to 0.051 ms per
30 frames, but transparent Gaussian processing still takes 30.530 ms. Off-screen
groups still compose their children: Gaussian time remains about 96–100 ms.
The effect cases record 60 scratch reuses per sample in both positions. The
copy-byte counter does not account for every direct image clone, so its zero
value is not evidence that these paths are copy-free. Generic subtree culling
is not implemented or claimed by this pass.

A separate evaluator benchmark performs five samples of 10,000 evaluations of
one scheduled root, with no explicit warmup. Advancing/repeated-time medians are
2.720/2.361 ms for animation-effects and 7.195/7.170 ms for nested-groups (eight
and 22 evaluated tracks per call). This measures evaluation including returned
frame construction/destruction, not allocation counts. Canonical timing reports
also attribute 0–5 rounded milliseconds per render to track evaluation. These
fixtures do not justify adding dynamic evaluation memoization; they do not prove
that evaluation is free or that larger projects cannot benefit.

## Group-matte correctness repair

The focused off-screen experiment exposed an existing scratch-contamination bug:
composing an effect-bearing child overwrote shared effect scratch, then an
isolated group used as a matte drew over those stale pixels. An off-screen group
could therefore leave a visible matte; a partially transparent centered group
could gain alpha. Clear scratch after child composition and before transforming
the completed group, as the ordinary group rendering path already does.

The regression failed before the fix and passes afterward for centered/off-screen
groups, alpha 64/128/255, and normal/inverted alpha mattes. Four hardware WGPU
comparisons with an effect-bearing child match CPU output exactly on GL through
D3D12 on the NVIDIA GTX 1650 SUPER. This is not exhaustive nested/luma-matte
coverage. The required full-frame clear adds work to uncached isolated group
mattes; it is a correctness fix, not a measured optimization.

## Rejected experiments

- Swapping complete effect buffers instead of copying passed pixel tests but
  regressed the paired combined workload from 6,631 to 6,794 ms. It was reverted.
- Skipping layers beneath an already cached opaque full-frame layer passed a
  constructed pixel/resource test but did not activate in the measured mixed
  image workloads. Cache misses remained 150/390/750 for 10/25/50 layers and no
  speedup appeared. It was reverted. Nested matte and decoder-error behavior
  would also have required further validation before keeping it.
- Cropping cached layer images cannot address the observed mixed-image workload:
  those cached outputs span the frame. Its LRU thrashing remains a limitation,
  not a solved issue.

## Reproduction and evidence

Use the canonical suite and comparison commands in [Performance](performance.md).
Do not compare prepared/null-sink timings with encoded one-shot timings. Broad
before/current suite timings drifted even on unaffected paths, so the isolated
paired measurements above drive retention decisions.

Focused commands:

```bash
cargo test --release -p vestra-render --no-default-features --features cpu --lib
cargo test --release -p vestra-render --no-default-features --features cpu cpu_work_elimination_benchmark -- --ignored --nocapture --test-threads=1
VESTRA_CPU_PROFILE=1 cargo test --release -p vestra-render --no-default-features --features cpu cpu_work_elimination_benchmark -- --ignored --nocapture --test-threads=1
cargo test --release -p vestra-render --no-default-features --features cpu cpu_animation_evaluation_benchmark -- --ignored --nocapture --test-threads=1
cargo test --release -p vestra-render --no-default-features --features cpu gaussian_bounds_benchmark -- --ignored --nocapture
cargo test --release -p vestra-render --no-default-features --features cpu spectrum2d_cpu_benchmark -- --ignored --nocapture
cargo test --release -p vestra-render --no-default-features --features cpu cpu_parallel_scaling_benchmark -- --ignored --nocapture
VESTRA_RENDER_BENCH=1 cargo test --release -p vestra --no-default-features --features cpu render_workload_matrix -- --nocapture --test-threads=1
```

The bounds reference binary used the same code and benchmark with convolution's
region fixed to `(0, width, 0, height)`. Sparse and full-frame implementations are
also compared against the separate scalar pixel reference, including a complete
two-pass blur, dirty transparent RGB, disconnected regions, edge pixels, radii
through 32, and reused target buffers.

Local raw artifacts are under `target/benchmark-results/cpu-efficiency-*` and the
decision trail is `.audit/cpu-efficiency.tsv`. These working artifacts are ignored
by Git. [Selected raw samples and counters](../../benchmarks/results/cpu-efficiency-2026-09.json)
are retained with this report; they are not a replacement canonical baseline. The checked-in baseline definitions were not changed.

`DISPLAY=:0 VESTRA_WGPU_BACKEND=gl ./scripts/check.sh` passed, including formatting,
all-feature workspace build, Clippy with warnings denied, Rust tests, schema
validation and schema freshness. Adapter discovery identified GL through D3D12 on
the NVIDIA GeForce GTX 1650 SUPER; Vulkan exposed CPU llvmpipe. The CPU-only SDK
unit run had one warning-count assertion failure that also reproduced in its
saved pre-worker-change binary. That test passes in the all-feature gate.
The Python authoring render, animation-frame, and transition/flash-frame suites
also passed after the group-matte repair: 24 tests against the rebuilt native
extension. The final-source canonical CPU suite completed all ten workloads and
50 samples. Against the preceding five-optimization suite, median changes ranged
from -3.83% to +1.11%, with overlapping raw timings and no material regression
identified. These sequential suite results confirm execution; isolated pairs
remain the evidence used to attribute optimization gains.
