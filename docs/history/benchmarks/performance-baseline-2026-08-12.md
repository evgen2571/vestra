# Vestra performance baseline and CPU profiling

This is the current release-mode baseline for the first performance
instrumentation phase. It is machine-specific evidence, not a performance
guarantee. No rendering algorithm was changed.

## Environment

| Item | Value |
| --- | --- |
| OS | Linux, kernel 7.0.0-27-generic, x86_64 |
| CPU | Intel Core i7-8750H, 6 physical cores / 12 logical threads |
| RAM | 14 GiB |
| GPU | No adapter reported by the available Vulkan/WGPU discovery tools |
| Rust / Cargo | rustc/cargo 1.96.1 |
| FFmpeg | 7.1.5 |
| Build | Cargo release profile |
| Auto CPU workers | 8 on the selected 1280x720 and 1920x1080 workloads; 2 at 3840x2160 under the existing frame-memory policy |

## Commands and method

Correctness was attempted before editing with:

```text
./scripts/check.sh
```

The fresh matrix used the retained renderer-only `NullSink` harness and the
existing end-to-end encoder path:

```text
VESTRA_PHASE10_BENCH=1 VESTRA_CPU_PROFILE=1 \
  cargo test --release -p vestra phase10_release_matrix --all-features -- --nocapture
```

It measured the existing cold/warm/warm-2 operations, fixed frame ranges,
and the existing fixture quality settings. The harness emitted one-line JSON
records prefixed with `vestra_benchmark`; the opt-in CPU breakdown was emitted
once per completed operation as `vestra_cpu_profile`.

## CPU renderer-only baseline

Selected warm rows from the fresh 41-row matrix:

| Scenario | Resolution | Frames | Workers | ms/frame | FPS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Pipeline floor | 1280x720 | 30 | 8 | 0.178 | 5619.83 |
| Pipeline floor | 1920x1080 | 30 | 8 | 0.421 | 2374.47 |
| Pipeline floor | 3840x2160 | 30 | 2 | 2.168 | 461.32 |
| Animated transform | 1280x720 | 100 | 8 | 17.909 | 55.84 |
| Multiple layers (10) | 1280x720 | 30 | 8 | 118.582 | 8.43 |
| Multiple layers (25) | 1280x720 | 30 | 8 | 566.771 | 1.76 |
| Multiple layers (50) | 1280x720 | 30 | 8 | 1096.805 | 0.91 |
| ZoomBlur transition | 1280x720 | 100 | 8 | 120.957 | 8.27 |
| Global post effect | 1280x720 | 30 | 8 | 0.187 | 5342.30 |
| Mixed realistic project | 1920x1080 | 30 | 8 | 197.931 | 5.05 |

The global-post warm row is cache-warm; its cold row was 7.823 ms/frame and
reported 7,372,800 explicit CPU copy bytes. Gaussian scaling remained the
existing focused harness at 1280x720: 0/1/3/5 passes measured 103.317,
222.573, 396.765, and 580.696 ms/frame respectively.

## CPU hot-path breakdown

Timings are aggregate worker CPU time. With multiple workers, aggregate time
can exceed wall time. Transform/sampling is a diagnostic subset of source
rasterization; it is not added to the total a second time.

| Scenario | Source raster / frame | Transform subset / frame | Effect / frame | Composition / frame |
| --- | ---: | ---: | ---: | ---: |
| Animated transform | 135.6 ms | 134.2 ms | 0 ms | ~0 ms |
| ZoomBlur transition | 178.3 ms | 178.3 ms | 676.2 ms | 22.7 ms |
| Gaussian, 1 pass | 136.4 ms | 136.3 ms | 123.6 ms | 32.7 ms |
| Gaussian, 3 passes | 134.6 ms | 134.6 ms | 364.8 ms | 32.6 ms |
| Gaussian, 5 passes | 131.0 ms | 131.0 ms | 579.1 ms | 32.9 ms |
| Mixed project | 322.9 ms | 322.9 ms | 1032.2 ms | 87.5 ms |

The profile also records separate Gaussian, ZoomBlur, Bloom/Glow, other
effect, global-post, and surface-copy durations. The mixed warm profile had
approximately 461.7 ms/frame of Bloom/Glow work, 570.5 ms/frame of other
effects, 571.0 ms/frame of global-post execution, and 2.4 ms/frame of surface
copy work in aggregate. Its existing counter reported 497,664,000 copy bytes
for the 30-frame operation.

## ZoomBlur transition decomposition

For the warm ZoomBlur workload, aggregate worker time per frame was:

| Category | Aggregate ms/frame |
| --- | ---: |
| Source rasterization / transform sampling | 178.3 |
| ZoomBlur effect kernel | 676.2 |
| Layer composition | 22.7 |
| Surface copy / other measured coarse work | ~0 |

The wall time was 121.0 ms/frame at eight workers. Therefore this workload is
not generic transition-framework overhead: the profile attributes the largest
measured share to ZoomBlur execution, with source transform work also present.

## Worker scaling

Existing ignored release benchmark, 1920x1080, 60 frames, same dynamic
workload:

| Workers | Actual workers | Wall ms | FPS | Aggregate frame-render ms |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 1 | 12253.831 | 4.90 | 12249.370 |
| 2 | 2 | 6212.677 | 9.66 | 12386.571 |
| 4 | 4 | 3213.744 | 18.67 | 12693.032 |
| auto | 8 | 2043.227 | 29.37 | 15879.274 |

## Renderer versus encoder

The current pipeline-floor comparison used the same 1280x720 fixture and 30
frames. Renderer-only cold execution was 1.217 ms/frame; the encoded path was
7.754 ms/frame. On this simple workload the end-to-end path is encoder-bound,
with roughly 6.5 ms/frame beyond the renderer-only measurement. The separate
animation-effects end-to-end run reported 17.09 ms/frame render wall time and
2.8 ms/frame encoder writes, so encoder cost is workload-dependent.

## WGPU and resource metrics

No hardware WGPU adapter was available, so no GPU performance conclusion or
hardware WGPU baseline is reported. Existing adapter-independent WGPU tests
remain part of the renderer suite.

The matrix preserved and reported existing cache/scratch/copy counters. The
selected warm 10/25/50-layer rows retained 88,473,600 scratch bytes at
1280x720; the mixed 1920x1080 row retained 199,065,600 bytes. The global-post
and mixed rows reported explicit copy traffic as noted above. WGPU readback
allocation counters remain unchanged and were not optimized.

## Overhead and limitations

The same release end-to-end benchmark (CPU, basic-colour, 720x1280, 144
frames, one warmup, three samples) measured:

| State | Median wall | Median frame-render work |
| --- | ---: | ---: |
| Before instrumentation | 2461 ms | 17504 ms |
| With instrumentation | 2477 ms | 17518 ms |

This is +16 ms / +0.65% wall time and +14 ms / +0.08% aggregate frame work.
Timing is coarse and uses worker-local `Instant` samples; no per-pixel
allocation, logging, formatting, mutex, or system call was introduced.

The source-rasterization bucket includes direct source drawing and therefore
can include source-path blending. Explicit `blend_surface` calls are measured
as composition. Surface-copy timing covers the effect-pool materialization
transfers, while the existing byte counters remain the authoritative view of
copy volume.

## Decision

| Target | Rank | Evidence |
| --- | --- | --- |
| CPU rasterization / transform sampling | Very High | 134–323 aggregate ms/frame in dynamic and mixed workloads; 10/25/50-layer scaling is dominated by source work |
| ZoomBlur | Very High | 676 aggregate ms/frame in the transition profile, the largest isolated class |
| Gaussian blur | High | 123.6, 364.8, and 579.1 aggregate ms/frame for 1/3/5 passes |
| Bloom/Glow and other effect kernels | High | Mixed workload measured ~461.7 and ~570.5 aggregate ms/frame respectively |
| Layer composition / blending | Medium | 22.7 ms/frame in transition and 87.5 ms/frame in mixed work |
| Full-frame copy / surface movement | Medium | 497,664,000 bytes in the mixed 30-frame operation; measured duration is smaller than kernel work |
| CPU worker scheduling | Low | 1/2/4/auto scaling remains strong: 4.90/9.66/18.67/29.37 FPS |
| WGPU execution/readback | Low | No hardware adapter available |
| Encoder | Low overall, workload-dependent | Dominant on the simple pipeline-floor comparison, but only ~2.8 ms/frame in animation-effects |

Recommended next subphase: optimize CPU rasterization / transform sampling,
using the ZoomBlur and Gaussian measurements as the next effect-specific
follow-up targets after that work is separately evaluated.

