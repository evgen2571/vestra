# CPU rasterization and transform-sampling subphase

## Scope

This audit covers the CPU dynamic image raster path and profiling-overhead cleanup only. No effect kernel, WGPU, worker-policy, encoder, public API, or schema changes are included.

## Method

The accepted pre-edit validation was `./scripts/check.sh`; it exited 0. Benchmarks used the release build, CPU backend, fixed fixtures and frame ranges, eight workers, and the existing two warm operations followed by `warm-2`. The harness command was:

```bash
VESTRA_PHASE10_BENCH=1 VESTRA_CPU_PROFILE=1 \
  VESTRA_PHASE10_BENCH_START=6 VESTRA_PHASE10_BENCH_LIMIT=1 \
  VESTRA_PHASE10_BENCH_OUTPUT=/tmp/vestra-after-animated-N.json \
  cargo test --release -p vestra phase10_release_matrix --all-features -- --nocapture
```

Five independent runs were used for the animated-transform before and after sets. Multi-layer and mixed before/after pairs used the same command and settings with the corresponding matrix index.

## Profiling cleanup

`VESTRA_CPU_PROFILE` is resolved once while constructing `CpuBackend` and passed as an immutable worker-local flag. CPU compositor, raster, and effect timers call `Instant::now()` only when that flag is enabled. No environment lookup, lock, or atomic operation is performed in the frame/pixel loops.

The three post-change non-profiled animated warm-2 samples were 16.506, 16.664, and 16.938 ms/frame (median 16.664). The five profiled warm-2 samples had median 16.739 ms/frame, an observed profile-on difference of approximately 0.45% against that small non-profiled set. Timing reports remain emitted when profiling is enabled.

## Implementation

The raster loop retains canonical inverse-affine scanline stepping, bilinear sampling, clipping, colour transform order, and source-over fallback. It adds one exact opaque-sample specialization:

```text
opacity == 1.0 && sampled_alpha == 255 -> write sampled pixel directly
otherwise -> existing source_over path
```

The branch uses the actual filtered sample alpha, not an inferred source opacity, so it remains valid at crop and image boundaries. All other samples use the canonical compositor.

## Results

### Animated transform, 1280x720, 100 frames, 8 workers

| Set | Median ms/frame | Median FPS | Samples |
| --- | ---: | ---: | ---: |
| Before | 17.942 | 55.736 | 5 |
| After | 16.739 | 59.742 | 5 |

Speedup is 1.072x, throughput improves 7.19%, and render time falls 6.71%.

Aggregate CPU timings per frame (median profiled warm-2 samples):

| Category | Before | After |
| --- | ---: | ---: |
| Source rasterization | 135.0 ms | 123.5 ms |
| Transform/sampling | 133.6 ms | 121.9 ms |
| Composition | approximately 0.0003 ms | approximately 0.0003 ms |

### Ten-layer dynamic workload, 1280x720, 30 frames, 8 workers

| Set | Median ms/frame | Median FPS | Samples |
| --- | ---: | ---: | ---: |
| Before | 118.957 | 8.406 | 1 |
| After | 85.449 | 11.703 | 1 |

Speedup is 1.392x, throughput improves 39.22%, and render time falls 28.16%. Static cache hits remained 150/0 in warm-2 and no new scratch allocation was recorded.

### Mixed workload, 1920x1080, 30 frames, 8 workers

| Set | Median ms/frame | Median FPS | Samples |
| --- | ---: | ---: | ---: |
| Before | 198.254 | 5.044 | 1 |
| After | 188.758 | 5.298 | 1 |

Speedup is 1.050x, throughput improves 5.03%, and render time falls 4.79%. Aggregate source rasterization fell from approximately 320.7 to 251.3 CPU-ms/frame in these controlled runs; effects remained dominant.

## Regression observations

The static-image warm-2 run remained cache-warm at 0 misses, 0 static renders, and 0 scratch allocations, with 0.155 ms/frame in the observed run. The ZoomBlur transition warm-2 run was 115.816 ms/frame, with approximately 666.7 aggregate CPU-ms/frame in ZoomBlur and approximately 138.5 aggregate CPU-ms/frame in source raster.

## Remaining bottleneck

Fresh profiles now rank ZoomBlur as the next target for the transition workload, followed by the mixed workload's other effects and global post-effect work. ZoomBlur optimization is not part of this subphase.
