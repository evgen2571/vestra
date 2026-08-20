# Phase 10D - final performance benchmark and audit

## Method

`phase10_release_matrix` is an internal test-only benchmark. It builds
temporary deterministic project files from existing fixtures, prepares the CPU
backend once, then measures a cold operation followed by two warm operations
through `NullSink`.
`NullSink` reads one byte per completed frame and writes an eight-byte checksum;
it never copies frame pixels or starts FFmpeg. The encoded comparison uses the
same pipeline-floor fixture and the normal FFmpeg sink.

Run the complete suite with:

```bash
VIDEO_EDITOR_PHASE10_BENCH=1 cargo test --release -p video-editor \
  phase10_release_matrix -- --nocapture
```

`VIDEO_EDITOR_PHASE10_BENCH_START` and `VIDEO_EDITOR_PHASE10_BENCH_LIMIT`
select a contiguous development subset. They are not a public API. Raw results
are in [phase10d-results.json](phase10d-results.json). The separate
[10-layer](phase10d-layer-results.json), [25-layer](phase10d-layer25-results.json),
and [50-layer](phase10d-layer50-results.json) files hold the expensive layer
scaling points. [Repeated prepared operations](phase10d-repeated-results.json)
and [random access](phase10d-random-results.json) are separate focused runs.
[Preparation results](phase10d-preparation-results.json) keep setup outside
the renderer-only frame timings. [Effect-pass results](phase10d-effect-results.json)
isolate 0, 1, 3, and 5 Gaussian passes on a dynamic source.

The suite uses 30 frames for floor, resolution, global-pass, mixed, and encoded
checks; 100 for cache-disabled, expensive, animated, effect, and transition
checks; and 300 for the steady-state static-image check. Each prepared operation
reports event deltas and ending resource gauges.

## Environment

| Item | Measured value |
| --- | --- |
| OS | Ubuntu Linux 7.0.0-27-generic, x86_64 |
| CPU | Intel Core i7-8750H, 6 cores / 12 threads |
| RAM | 14 GiB |
| Rust | 1.96.1 |
| Python | 3.13.5 |
| FFmpeg / FFprobe | 7.1.5 |
| Build | Cargo release profile |
| WGPU | No compatible adapter was available (`WGPU-ADAPTER-NOT-FOUND`) |

No historical pre-Phase-10 release baseline was reproducible locally. This
audit therefore reports current measurements, cache-disabled references, and
cold versus warm structural deltas instead of invented before/after gains.

## Preparation

| Workload | Resolution | Preparation ms |
| --- | ---: | ---: |
| Pipeline floor | 1280x720 | 99.24 |
| Animated transform | 1280x720 | 0.47 |
| Short mixed | 1920x1080 | 0.33 |

These are `Project -> PreparedState` timings. They include current validation,
planning, decode, and CPU backend setup. They are deliberately separate from
per-frame renderer throughput.

## CPU renderer-only results

| Workload | Resolution | Frames | Execution | FPS | ms/frame |
| --- | ---: | ---: | --- | ---: | ---: |
| Pipeline floor | 1280x720 | 30 | cold / warm | 37.96 / 39.23 | 26.34 / 25.49 |
| Pipeline floor | 1920x1080 | 30 | cold / warm | 16.62 / 16.07 | 60.16 / 62.24 |
| Pipeline floor | 3840x2160 | 30 | cold / warm | 4.17 / 4.33 | 239.74 / 230.97 |
| Static image | 1280x720 | 300 | cold / warm | 38.85 / 39.04 | 25.74 / 25.62 |
| Static expensive Gaussian blur | 1280x720 | 100 | cold / warm | 35.41 / 39.32 | 28.24 / 25.43 |
| Animated transform | 1280x720 | 100 | cold / warm | 9.49 / 9.50 | 105.36 / 105.31 |
| Dynamic effects | 1280x720 | 100 | cold / warm | 8.02 / 8.03 | 124.68 / 124.43 |
| ZoomBlur-focused | 1280x720 | 100 | cold / warm | 1.65 / 1.65 | 605.22 / 606.00 |
| Global post effect | 1280x720 | 30 | cold / warm | 8.09 / 8.26 | 123.57 / 121.01 |
| Short mixed | 1920x1080 | 30 | cold / warm | 1.03 / 1.02 | 973.19 / 979.27 |

Resolution cost follows pixel count closely: warm 720p, 1080p, and 4K floor
times are 25.49, 62.24, and 230.97 ms. The 1080p to 4K ratio is 3.71 for a
four-times pixel increase, so the floor is dominated by full-frame CPU work,
not fixed evaluator overhead.

## Cache and allocation evidence

| Workload | Cache mode | Hits | Misses | Static renders | Scratch allocs | Scratch reuses |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Static image, cold | enabled | 299 | 1 | 1 | 1 | 0 |
| Static image, warm | enabled | 300 | 0 | 0 | 0 | 0 |
| Static image | disabled | 0 | 100 | 100 | 0 | 0 |
| Static expensive, cold | enabled | 99 | 1 | 1 | 1 | 1 |
| Static expensive, warm | enabled | 100 | 0 | 0 | 0 | 0 |

Cache-disabled static image rendering took 126.67 ms/frame, against 25.62
ms/frame warm cached. Static reuse eliminates 100 physical static renders in
that 100-frame reference. The static expensive chain shows the same one-render
population rule and stays at 25.43 ms/frame warm.

The repeated 300-frame static-image operation measured 27.63 ms/frame cold,
27.10 ms/frame warm, and 26.60 ms/frame on the second warm run. Its cache
counters are 299/1 hits/misses, then 300/0 and 300/0; scratch allocation is
one, then zero and zero. The prepared backend retains its cache and workspace
across operations as intended.

The three CPU scratch surfaces remain bounded. At 720p their retained logical
size is 11,059,200 bytes; at 1080p it is 24,883,200 bytes; at 4K it is
99,532,800 bytes. Dynamic Gaussian work recorded scratch reuse without new
scratch allocation after preparation. The 100-frame over-budget regression in
the CPU suite independently proves three allocations, 100 bypasses, zero cache
entries, and pixel equality with the dynamic reference.

Remaining major CPU copies are completed-frame ownership and post-effect
canvas/scratch transfers. Global-post, many-layer, and mixed rows report
221,184,000, 221,184,000, and 497,664,000 explicit post-effect copy bytes
respectively. These are required by the current independent-frame ownership
and ping-pong algorithm; their size tracks full-frame passes.

## Layer scaling

The layer tests use 50% static image layers and 50% animated image layers at
1280x720 for 30 frames. The first warm operation populates each static cache;
the second warm operation confirms the steady state.

| Layers | Warm ms/frame | Static hits / misses | Interpretation |
| ---: | ---: | ---: | --- |
| 10 | 599.08 | 150 / 0 | Composition and five dynamic layers dominate. |
| 25 | 1,585.72 | 390 / 0 | Cost grows roughly with active layers. |
| 50 | 3,376.86 | 750 / 0 | Still near-linear, with full-frame dynamic rasterization dominant. |

The 10-to-50 mixed static/dynamic layer ratio is 5.64 in time for five times as many layers. There
is no cache miss or scratch-allocation growth after the first operation.

## Effect scaling

| Gaussian passes | FPS | ms/frame | Scratch reuses |
| ---: | ---: | ---: | ---: |
| 0 | 9.68 | 103.32 | 0 |
| 1 | 4.49 | 222.57 | 30 |
| 3 | 2.52 | 396.77 | 90 |
| 5 | 1.72 | 580.70 | 150 |

The first Gaussian pass adds about 119 ms/frame. Additional passes add roughly
87 to 92 ms/frame. This is the strongest direct evidence in the matrix for an
effect-algorithm bottleneck, and it is why a faster blur algorithm is a future
recommendation rather than an unmeasured assumption.

## Random access

| Workload | Frame | Cold/warm | ms |
| --- | ---: | --- | ---: |
| Static image | 0 / 99 | cold / warm | 133.43 / 27.28 |
| Animated transform | 0 / 99 | cold / warm | 92.65 / 208.81 |

Static random access benefits from the same complete-layer cache. Animated
random access does not: frame 99 includes actual transform/effect work and is
slower than its first-frame counterpart.

## Renderer versus encoding

The 1280x720 pipeline floor was 0.790 s renderer-only cold and 0.953 s encoded
for 30 frames. The approximate FFmpeg overhead is 0.163 s, or 5.42 ms/frame.
The floor is renderer-dominated on this machine. The encoded run used the
repository's normal FFmpeg configuration and disabled audio to isolate video.

## WGPU

WGPU runtime benchmarks were not run because this machine has no compatible
adapter. The adapter-independent WGPU planning, readback, and resource tests
compile and run. On an adapter-backed host, the existing WGPU benchmark path is:

```bash
VIDEO_EDITOR_BENCH_BACKEND=wgpu VIDEO_EDITOR_REQUIRE_WGPU=1 \
cargo bench -p video-editor --bench animation_effects -- --nocapture
```

Run it for the same effect fixtures and record the selected adapter, command
submission, readback allocation, row-repack, static-cache, and prepared-slot
metrics. This CPU-only environment has no WGPU numbers to compare.

`wgpu_temporary_texture_reuses` means prepared-working-texture reuse slots: it
adds the fixed prepared working-texture count for every submission after the
first. It does not claim every texture participated in that frame. WGPU
readback still requires padded mapped rows to be repacked into an independently
owned tight RGBA frame.

## Decision

Phase 10A reduces compiler-side work through identity removal, fusion, and
static/dynamic classification. Phase 10B removes repeated static layer work.
Phase 10C bounds CPU/GPU workspace and records ownership and copy costs. Phase
10D now measures those paths in release mode.

The largest remaining CPU bottlenecks are transition processing and large
multi-layer/post-effect compositions, not static caching or scratch allocation.
The next optimization phase should profile those effect algorithms and
full-frame copies before considering SIMD, parallelism, or a new cache layer.
Hardware encoding, frame parallelism, GPU encoder interop, and a new blur
algorithm are not justified by this single CPU-only data set.
