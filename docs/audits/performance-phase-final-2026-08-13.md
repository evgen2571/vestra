# Vestra performance phase final audit — 2026-08-13

## 1. Scope and goals

This is the authoritative close-out for the current CPU performance phase. It
consolidates the accepted CPU work, records rejected experiments, standardizes
benchmark terminology and profiler units, and hands future WGPU measurement to
a real-hardware environment. No new CPU optimization hunt or speculative WGPU
change is part of this phase.

Claims in this audit use matched same-session before/after measurements when a
comparison is made. The current benchmark snapshot is a reference baseline,
not a cumulative speedup calculation.

## 2. Benchmark methodology

The internal Phase 10 harness is run with:

```bash
VESTRA_PHASE10_BENCH=1 cargo test --release -p vestra \
  phase10_release_matrix --all-features -- --nocapture
```

It prepares a fixture, renders through the CPU backend and a null sink, and
reports cold, warm, and warm-2 operations. `VESTRA_PHASE10_BENCH_START` and
`VESTRA_PHASE10_BENCH_LIMIT` select a development subset. The focused
`animation_effects` benchmark uses two warmups and five measured samples:

```bash
VESTRA_BENCH_BACKEND=cpu VESTRA_BENCH_SCENARIO=combined \
  cargo bench --release -p vestra --bench animation_effects -- --nocapture
```

The Phase 10 rows are renderer-only and use the existing null sink. Encoded
pipeline measurements are separate. A warm-2 row is a regression snapshot; a
five-sample median is the preferred headline comparison.

## 3. Profiler semantics and units

`vestra_cpu_profile` counters are aggregate worker CPU duration across the
complete render operation. They are not wall time and are not inherently
per-frame. Reports therefore use:

```text
aggregate CPU duration: X ms
rendered frames: N
normalized: X / N CPU-ms/frame
```

Exclusive ranking percentages use only non-overlapping concrete timers in the
listed total. Inclusive parents such as `effect_execution`, `global_post_effect`,
`bloom_glow`, and `sharpen` are excluded from that denominator. Percentages are
rounded only after calculating the shares.

## 4. Canonical workload definitions

| Scenario | Purpose and contract |
| --- | --- |
| `pipeline-floor` | Simple renderer floor; 720p, 1080p, and 4K rows are resolution checks. |
| `static-image` | Cache-warm static image; 300 frames. |
| `animated-transform` | Dynamic image with animated transform; 100 frames. |
| `mixed-layers-10` | Mixed static/dynamic ten-layer workload; 50% static and 50% animated clips, 30 frames. |
| `color-adjust-focused` | Isolated Color Adjust workload; 100 frames. |
| `chromatic-focused` | Isolated Chromatic Aberration workload; 100 frames. |
| `zoom-blur-focused` | ZoomBlur transition workload; 100 frames. |
| `gaussian-focused` | Isolated Gaussian workload; 100 frames. |
| `bloom-focused` | Reserved canonical name for a dedicated Bloom fixture; no current Phase 10 fixture exposes Bloom alone, so no value is fabricated. |
| `short-mixed` | 30-frame general mixed early-timeline regression; it may not reach ZoomBlur or Chromatic Aberration. |
| `long-combined` | 180-frame full effects-ready project; primary end-to-end and bottleneck-ranking workload. |

The harness currently emits the stable names `pipeline-floor`, `static-image`,
`animated-transform`, `mixed-layers-10`, `color-adjust-focused`,
`chromatic-focused`, `zoom-blur-focused`, `gaussian-focused`, and
`long-combined`. `short-mixed` is the documentation name for the 30-frame
mixed row. The dedicated `bloom-focused` and `Spectrum2D` rows remain future
matrix entries until a current fixture can isolate them without changing
benchmark behavior.

The ten-layer case is explicitly a mixed static/dynamic workload, not a purely
dynamic ten-layer workload. Short mixed and long combined are not substitutes:
short mixed is useful for early-timeline regression, while long combined reaches
later effects and is the representative full-project ranking workload.

## 5. CPU architecture status

The established renderer already has persistent CPU workers, frames in flight,
worker-local state and caches, bounded surface pools, out-of-order backend
completion with ordered delivery, cancellation/failure cleanup, and
random-access frame rendering. CPU multicore execution must not be
reimplemented in a future performance phase.

## 6. Accepted optimizations

| Optimization | Target | Result | Correctness | Status |
| --- | --- | --- | --- | --- |
| Raster Stage 1 | Exact opaque-sample composition shortcut | 17.942 → 16.739 ms/frame; about 7.2% throughput improvement on matched animated-transform work | Exact | Accepted |
| ZoomBlur Stage 1 | Precomputed factors and specialized bilinear sampling | 113.674 → 98.806 ms/frame; about 15.1% throughput improvement | Sample count and byte output preserved | Accepted |
| ZoomBlur Stage 2 | Safe raw RGBA slice sampling | 98.734 → 96.304 ms/frame; about 2.53% throughput improvement | Exact; corrected pre-value profiler median is 552.291 | Accepted |
| Color Adjust LUT | One local `[u8; 256]` LUT per evaluated pass | 26.010 → 20.213 ms/frame; about 28.7% throughput improvement; focused kernel reduction about 94.2% | Exact | Accepted |
| Chromatic Aberration | Selected-channel bilinear sampling and safe RGBA access | 2.343 → 1.959 ms/frame; about 19.6% throughput improvement | Alpha-aware semantics preserved | Accepted |
| Alpha Composition | Transparent effective-source fast path and hoisted Normal dispatch | 232.553 → 230.097 ms/frame; about 1.07% throughput improvement; composition aggregate 28,244.841 → 27,492.963 ms over 60 frames | Exhaustive/reference byte equality | Accepted |

These figures are workload-specific and are not arithmetically additive.

## 7. Rejected experiments

| Experiment | Why tested | Measurement and decision |
| --- | --- | --- |
| Raster raw-slice sampler | Reduce image-neighbor access overhead | Regressed matched animated-transform work by about 12%; reverted. |
| Raster manual crop/dimension invariant hoisting | Remove repeated invariant work | Regressed the same workload by about 12%; reverted. |
| Gaussian opaque specialization | Avoid work for opaque samples | Exact micro-gains were only about 0.6–1.2%; no production change retained. |
| Other Gaussian/Bloom exact experiments | Test small kernel simplifications | Diminishing-return territory; downsampling, box blur, and alternative large-radius algorithms require explicit quality/parity decisions and were not implemented. |

## 8. Current representative CPU baselines

The current Phase 10 release snapshot was collected on this final production
state with the CPU backend and null sink. These are warm-2 regression values
unless marked as a repeated focused result; wall sessions are not presented as
cross-session A/B comparisons.

| Scenario | Resolution / frames | Warm-2 ms/frame | FPS | Measurement class |
| --- | --- | ---: | ---: | --- |
| `pipeline-floor` | 1280×720 / 30 | 0.856 | 1,168 | single warm-2 snapshot |
| `static-image` | 1280×720 / 300 | 1.129 | 886 | single warm-2 snapshot |
| `animated-transform` | 1280×720 / 100 | 74.121 | 13.491 | single warm-2 snapshot |
| `gaussian-focused` | 1280×720 / 100 | 179.769 | 5.563 | single cold regression snapshot; cache-disabled focused row |
| `composition-focused` | 1280×720 / 60 | 230.097 | 4.346 | matched five-run after median from focused audit |
| `mixed-layers-10` | 1280×720 / 30 | 98.654 | 10.136 | matched five-run after median from focused audit |
| `short-mixed` | 1920×1080 / 30 | 979.680 | 1.021 | single warm snapshot |
| `long-combined` | 1920×1080 / 180 | 245.466 | 4.074 | matched five-run after median from focused audit |

The older Phase 10 resolution reference remains 25.49 ms/frame at warm 720p,
62.24 ms/frame at warm 1080p, and 230.97 ms/frame at warm 4K for the pipeline
floor. It is retained as a resolution reference, not mixed into the focused
snapshot above. No Bloom-only current value is claimed.

## 9. Final long-combined profile

The final 180-frame profile uses non-overlapping exclusive counters. Aggregate
durations were divided by 180, and the exclusive denominator is
`1922.490 CPU-ms/frame`:

| Exclusive category | CPU-ms/frame | Share |
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

The displayed shares sum to approximately 100% after rounding. Inclusive
parent metrics are not in this denominator.

## 10. Measurement caveats

Aggregate profiler values from 30-, 100-, 150-, and 180-frame operations must
not be compared without frame normalization. Wall measurements from different
sessions are not controlled A/B results. CPU and WGPU numbers from different
machines must not be turned into a direct speedup ratio. The CPU renderer is
not “fully optimized”; current exact hot-path work has reached diminishing
returns under the measured workloads.

## 11. CPU stopping rationale

The largest easy exact wins have been captured. Recent CPU micro-passes return
roughly 1–3% or no gain, and plausible raster experiments regressed and were
reverted. Gaussian/Bloom material gains require quality or algorithm decisions.
Credible WGPU optimization requires hardware unavailable in this environment.
Feature development therefore has higher expected value now. Future CPU work
should begin only when a new real workload supplies new profiling evidence.

## 12. WGPU measurement status

This agent environment has no usable WGPU adapter (`WGPU-ADAPTER-NOT-FOUND`).
This phase makes no claim about GPU performance, CPU/WGPU crossover, WGPU
speedup, readback cost, shader bottlenecks, or texture-upload bottlenecks.
No speculative WGPU production optimization was implemented.

## 13. Future WGPU benchmark procedure

On a GPU-capable machine:

1. Detect and record the adapter with the existing strict backend path.
2. Run the CPU comparison on that same machine.
3. Run the same scenario with WGPU and require actual WGPU selection.
4. Collect the benchmark report and profiler output, including adapter/backend,
   frame count, wall median/range, renderer timing, submission, readback, row
   repack, cache, slot, and allocation metrics.
5. Compare only matched project, resolution, frame range, quality, release
   build, and output semantics.

Current commands are:

```bash
# CPU, repeated renderer/encoded timing harness
VESTRA_BENCH_BACKEND=cpu VESTRA_BENCH_SCENARIO=combined \
  cargo bench --release -p vestra --bench animation_effects -- --nocapture

# WGPU, strict adapter selection; fails instead of falling back
VESTRA_BENCH_BACKEND=wgpu VESTRA_REQUIRE_WGPU=1 \
  cargo bench --release -p vestra --bench animation_effects -- --nocapture

# Explicit CLI adapter-backed render/report check
VESTRA_REQUIRE_WGPU=1 cargo run --release -p vestra-cli -- render \
  examples/projects/effects-ready-v1.json --render-backend wgpu \
  --output /tmp/vestra-wgpu-check.mp4 --overwrite
```

The final command is an adapter-backed report/render check, not a replacement
for the repeated benchmark. If a command is unavailable in a future checkout,
record it as missing rather than adding speculative benchmark infrastructure.

## 14. Future WGPU benchmark matrix

Run 720p, 1080p, and 4K where supported for static image, animated transform,
mixed layers, Color Adjust, Chromatic Aberration, ZoomBlur, Gaussian, Bloom,
and long combined. Add Spectrum2D using its existing example when the required
audio/example setup is available. Particles are not a current benchmark and
are intentionally omitted.

Measure total wall ms/frame and FPS; CPU-side submission, GPU completion when
measurable, texture upload, readback/map, row repack, resource allocation and
reuse, readback-slot count, texture create/reuse count, and relevant encoder or
output overhead.

Before any performance decision, verify CPU/WGPU effect output parity, alpha
correctness, random access, failure behavior, and deterministic frame
semantics. No WGPU performance optimization should be accepted from static
inspection alone; shader, resource, readback, upload, pipeline, and command
submission changes require real before/after GPU measurements.

## 15. Benchmark/profiler infrastructure

The useful infrastructure remains: opt-in coarse CPU timing categories,
worker-local aggregation, composition case counters when profiling is enabled,
the Phase 10 benchmark scenarios, random-access checks, cache/scratch metrics,
and the existing WGPU adapter-gated reporting. The disabled-profiling
composition loop no longer evaluates a per-pixel case-counter branch.

## 16. Recommended next project phase

Recommend **Particle System / Procedural Visuals**. Vestra is ready for that
feature phase with multicore CPU execution, worker-local state, deterministic
frame evaluation, effect infrastructure, audio-reactive parameters, profiling,
and the Spectrum2D precedent. Particle implementation is not part of this
audit.

For expensive future rendering features, require a focused benchmark, a
long-combined regression check, a memory-bound check, and determinism/
correctness tests before merging substantial renderer additions.

## Validation and compatibility

The required targeted checks are:

```bash
cargo test -p vestra-render --all-features blend::tests::
cargo test -p vestra-render --all-features cpu::compositor::tests::
cargo test -p vestra-render
cargo test -p vestra
./scripts/check.sh
```

Final `./scripts/check.sh` completed with exit 0 after serialized validation
(`CARGO_BUILD_JOBS=1` was used once to avoid linker resource contention; the
subsequent exact command also completed through schema generation). The
composition reference tests cover transparent-source behavior,
transparent-black canonicalization, opacity, source-over arithmetic,
rounding/clamping, Normal and non-Normal modes, and complete surface byte
equality. Profiling-enabled counters remain in the worker-local report path.

Rust public API: unchanged. Python public API: unchanged. CLI contract:
unchanged. Project JSON/schema: unchanged. No crate boundary or worker
architecture redesign occurred. No particles, generated media, or unrelated
refactor was added.

## Audit/documentation trail

This document is the primary starting point for future performance work. It
references the focused audits:

* `cpu-rasterization-2026-08-12.md`
* `cpu-source-raster-stage2-2026-08-13.md`
* `cpu-zoom-blur-2026-08-12.md`
* `cpu-zoom-blur-stage2-2026-08-13.md`
* `cpu-color-adjust-lut-2026-08-12.md`
* `cpu-chromatic-aberration-2026-08-13.md`
* `cpu-gaussian-bloom-algorithm-2026-08-13.md`
* `cpu-alpha-composition-2026-08-13.md`

## Performance phase status

**The current CPU performance phase is complete.** Future CPU performance work
must be driven by new profiling evidence. Future WGPU optimization requires
real hardware measurements and same-machine CPU comparison.

Finalization scope confirmation: no new CPU optimization hunt; no source-raster,
ZoomBlur, Chromatic Aberration, Color Adjust, Gaussian/Bloom, or Vignette
optimization; no speculative WGPU optimization; no worker-policy change; no
particles/features; and no unrelated refactor.
