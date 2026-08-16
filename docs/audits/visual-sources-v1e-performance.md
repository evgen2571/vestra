# Visual Sources v1E performance audit

Date: 2026-08-16. Repository state: `eaa34da32946083ff01df4684ff651b72335e0cb`.

## Summary

This pass adds decoder request/decode/seek/cache counters, WGPU Video upload
count/bytes, all-backend adapter discovery, and explicit backend selection in
the WGPU verification scripts. No speculative renderer optimization was
justified. The minimal native dependency repair updates `ffmpeg-next` from
7.1.0 to 8.0.0 (Cargo resolves 8.1.0), allowing the SDK benchmark and
workspace checks to build.

## Repository findings

Static images, Shapes, and Text are prepared once. CPU workers retain scoped
Video decoder sessions. WGPU opens only used Video assets, retains one decoder
per used asset, allocates compact compiled Video slots, reuses textures, and
uploads only when selected PTS changes. Render and encoder timings are already
separate.

## WGPU adapter discovery

Command: `cargo run --profile release -p vestra-render --example wgpu_adapters --all-features`

| Backend | Adapter | Device type | Classification |
| --- | --- | --- | --- |
| Vulkan | `llvmpipe (LLVM 21.1.8, 256 bits)` | `Cpu` | Software fallback |
| GL | `D3D12 (NVIDIA GeForce GTX 1650 SUPER)` | `Other` | Hardware, corroborated by accelerated GLX |

`nvidia-smi` reported the same NVIDIA GPU. `glxinfo -B` reported Microsoft
D3D12, NVIDIA, and `Accelerated: yes`. Vulkan reported only llvmpipe.

## Selected WGPU backend

Backend: GL
Adapter: `D3D12 (NVIDIA GeForce GTX 1650 SUPER)`
Device type: `Other` (WGPU-reported)
Hardware or software: Hardware through WSL D3D12/NVIDIA GL
Why selected: GL exposes the real NVIDIA adapter; Vulkan exposes CPU llvmpipe.

## Hardware validation status

FAILED for the complete renderer suite. Focused Video tests passed, but the
full GL run had broad parity failures and ended with SIGSEGV. The GL benchmark
did execute on the externally corroborated NVIDIA/D3D12 path, but WGPU reports
the adapter class as `Other`; those timings are diagnostic hardware-backed
measurements, not a full correctness or performance sign-off.

## Software fallback

Vulkan/llvmpipe was discovered but not used as default or for performance
claims. No software-WGPU performance result is retained.

## Integration regressions

Existing v1E coverage remains: real VFR, `source_start`, `playback_rate`,
random access, same asset/different PTS, nested timing, worker parity, mixed
sources, and Video timing. The focused GL group passed 4/4:

```text
gpu_video_crop_matches_cpu_for_a_dynamic_frame_when_an_adapter_is_available
gpu_video_preparation_ignores_unused_assets_when_an_adapter_is_available
gpu_video_source_timing_matches_cpu_when_an_adapter_is_available
gpu_video_to_video_transition_advances_both_sources_and_matches_cpu
```

The generated mixed-dynamic workload exercises two Videos with
`source_start`, `playback_rate`, a Video↔Video opacity transition, Text, Shape,
and a nested Group. The focused GL Video tests passed 4/4; the complete GL
suite remains failed as above. An isolated rerun of the transition test can
still terminate with the same GL/D3D12 SIGSEGV, so this is not full-suite
hardware sign-off.

## Benchmark environment and workloads

WSL; Rust stable, Cargo `1.97.1`; FFmpeg `8.0.1-3ubuntu2`; libavcodec
`62.11.100`, libavformat `62.3.100`, libavutil `60.8.100`, libswscale `9.1.100`.
The benchmark target was 320×180, release profile, one warmup and three
measured samples for CPU and GL. CPU used the automatic eight-worker backend
(`actual_backend_pipeline_depth=8`) on an AMD Ryzen 5 3600 6-Core Processor
with 12 logical CPUs. The benchmark writes raw RGBA frames through FFmpeg
using `libx264`, `ultrafast`, CRF 30, and `yuv420p`; render and encoder timings
are reported separately.

Workload A is the existing static-heavy composition/effects fixture. Workload
B generates one lossless FFV1 Video and overlays Text, Shape, and brightness.
Workload C generates two lossless FFV1 Videos, source timing, a transition,
Text, Shape, and nested Group content. The generated media is created by the
benchmark with `ffmpeg`, so no repository media fixture is required.

## Baseline results

These are actual retained CPU release measurements from
`docs/audits/performance-phase-final-2026-08-13.md`; they are not new A/B
claims:

| Reference | Backend | Resolution / frames | Warm-2 ms/frame | FPS |
| --- | --- | --- | ---: | ---: |
| pipeline floor | CPU | 1280×720 / 30 | 0.856 | 1168 |
| static image | CPU | 1280×720 / 300 | 1.129 | 886 |
| mixed layers | CPU | 1280×720 / 30 | 98.654 | 10.136 |
| long combined | CPU | 1920×1080 / 180 | 245.466 | 4.074 |

Fresh CPU results (median of three measured renders) were:

| Workload | Frames | Wall | Render | Frame render | Encode write | Metrics |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| A static-heavy | 144 | 258 ms | 258 ms | 930 ms aggregate | 123 ms | 0 sessions, 0 opens |
| B single Video | 90 | 214 ms | 211 ms | 687 ms aggregate | 84 ms | 8 sessions/opens, 90 requests, 320 decodes, 0 hits, 90 misses, 129,248 µs decode |
| C mixed dynamic | 90 | 258 ms | 254 ms | 1,121 ms aggregate | 71 ms | 16 sessions/opens, 119 requests, 533 decodes, 0 hits, 119 misses, 222,889 µs decode |

Fresh GL WGPU results were the median of three measured samples after one
warmup:

| Workload | Wall | Render | Frame render | Encode write | GPU init | Uploads / bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| B single Video | 902 ms | 827 ms | 214 ms | 37 ms | 252 ms | 40 / 9,216,000 |
| C mixed dynamic | 914 ms | 846 ms | 240 ms | 34 ms | 253 ms | 62 / 14,284,800 |

These GL numbers use the real GL adapter identified above, but are not a
claim that the full hardware WGPU renderer is correct because the complete
suite fails.

The corresponding GL Video metrics were B: 1 retained session, 1 decoder
open, 91 requests, 42 actual decodes, 50 cache hits, 41 misses, and 15,087 µs
decode time; C: 2 retained sessions/opens, 121 requests, 70 actual decodes,
57 hits, 64 misses, and 24,285 µs decode time. The 15-fps generated sources
demonstrate held-frame behavior at the 30-fps output rate: repeated PTS
selections are cache hits and do not trigger uploads.

## Instrumentation and profiling

Added `VideoDecoderMetrics` for frame requests, actual decodes, seeks, cache
hits/misses, and decode microseconds; CPU and WGPU aggregate these metrics.
WGPU now records decoder opens, retained sessions, Video upload count, and
bytes. Existing render, encoder,
initialization, submission, readback, cache, slot, and resource metrics remain.

The opt-in `VESTRA_CPU_PROFILE=1` run on B reported 605.4 ms aggregate source
rasterization across eight workers versus 15.6 ms layer composition and no
effect-pass time; decoder accounting reported 128,394 µs for 90 requests.
This identifies source rasterization/Video preparation as the dominant measured
CPU category for B. The retained effect profile still ranks ZoomBlur, Gaussian
passes, and Chromatic Aberration for the effect-heavy static workload. WGPU B
reported 8 ms texture upload timing, 6 ms command encoding, and 178 ms
submission timing; no additional upload optimization was justified beyond the
existing PTS check.

## Existing optimizations

Unused decoder avoidance remains structural: WGPU opens only used assets, and
the focused test passes. Initial same-PTS upload avoidance remains structural;
the new counter makes it measurable. The optimized behavior has a controlled
temporary-baseline comparison. On the same GL adapter, 320×180, one warmup,
and three measured samples:

| Optimization / workload | Before | After | Difference |
| --- | --- | --- | --- |
| Initial PTS tracking / B uploads | 41 uploads / 9,446,400 bytes; 894 ms wall; 822 ms render | 40 / 9,216,000 bytes; 902 ms wall; 827 ms render | −1 upload / −230,400 bytes; timing within run variance |
| Decoder deduplication / `dedup_video` | 2 decoder opens; 910 ms wall; 837 ms render | 1 decoder open; 897 ms wall; 827 ms render | −1 open; timing not treated as significant |

The baseline was created only in a temporary working-tree edit and restored;
no slower baseline code or toggle remains. CPU A creates zero Video sessions.
The structural reductions are valid, but no percentage performance gain is
claimed. No additional optimization was implemented.

Sequential and held-frame measurements are above. The native media
random-access fixture sequence (`2.1, 0.1, 1.1, 2.1, 2.49, 0.1` seconds)
produced six requests, six actual decodes, one miss, five cache hits, zero
seeks, and 203 µs decode time in the retained run.
Sessions remain backend/worker scoped;
zero-Video projects do not create Video sessions; cache budgets remain bounded.

## CPU and WGPU performance

CPU: fresh A/B/C measurements are recorded above.
Hardware-backed WGPU GL: diagnostic B/C measurements are recorded above; full
correctness remains failed.
Software WGPU: correctness fallback only; no performance claim.

## Static and mixed regressions

Renderer tests preserve static-only preparation and existing mixed integration
coverage. Generated C provides fresh end-to-end CPU and GL timing. Broad GL
parity remains FAILED as described above.

## Before/after summary

| Item | Before | After | Result |
| --- | --- | --- | --- |
| Decoder observability | No request/decode/seek counters | Counters in render stats | Added |
| WGPU Video upload observability | Test-only count | Production count and bytes | Added |
| Verification backend default | Vulkan default in scripts | Explicit backend required | Prevents accidental llvmpipe |
| Unused decoder optimization | v1E implementation | Retained | Structural focused test passes; no percentage claim |
| Initial upload optimization | v1E implementation | Retained and counted | 40 uploads / 90 GL frames in B; −1 upload versus temporary baseline |

## Resource behavior

Prepared static raster resources remain one-time. WGPU B retained one dynamic
Video session/open and 3,061,200 bytes of working resources in the duplicate-
slot diagnostic; C retained two sessions/opens and 3,565,840 bytes. WGPU
textures and compact per-slot resources are reused. The new counters expose
session/cache/decode/upload behavior in the current native benchmark runs.

## Retained performance report

This document: `docs/audits/visual-sources-v1e-performance.md`.

## Existing regressions

v1D1 media: retained and media tests pass. Image, Shape, Text, Video,
Spectrum2D, ParticleSystem, Groups, and CPU transition tests remain in the
renderer suite. Focused GL Video tests passed. Full GL parity failed. Audio
and SDK-level Rust tests pass after the FFmpeg binding update. Python pytest
passes with the declared uv `test` extra; Ruff reports 795 pre-existing
repository lint findings and was not changed.

## Validation commands

| Command | Status |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo check --workspace --all-features` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test -p vestra-media --all-features` | passed; 97 passed, 3 ignored |
| WGPU adapter example | passed; two adapters listed above |
| `VESTRA_WGPU_BACKEND=gl cargo test --profile release -p vestra-render --all-features gpu_video -- --nocapture` | passed; 4/4 |
| `VESTRA_WGPU_BACKEND=vulkan cargo test --profile release -p vestra-render --all-features gpu_video_to_video_transition_advances_both_sources_and_matches_cpu -- --nocapture` | passed; 1/1 software-WGPU correctness |
| `VESTRA_WGPU_BACKEND=gl cargo test --profile release -p vestra-render --all-features gpu_video -- --nocapture` | passed; 4/4 focused Video tests including Video↔Video transition; isolated transition rerun remains SIGSEGV-prone |
| Full GL renderer tests | failed; parity failures and SIGSEGV |
| Full Vulkan/llvmpipe renderer tests | individual tests reached 314/314 successfully, process then SIGSEGVed; correctness fallback not a clean suite pass |
| SDK CPU benchmark A/B/C | passed; measurements above |
| SDK GL WGPU benchmark B/C | executed on selected GL path; diagnostic only |
| Workspace tests / `scripts/check.sh` | failed in broad GL renderer tests; parity failures and SIGSEGV |
| `uv run --extra test pytest` | passed |
| `uv run --with ruff ruff check .` | failed; 795 existing findings |
| `python3 -m compileall python/vestra` | passed |

## WGPU validation

Selected backend: GL
Adapter: `D3D12 (NVIDIA GeForce GTX 1650 SUPER)`
Device type: `Other`
Hardware/software: Hardware, independently corroborated
Hardware correctness: FAILED for full suite; focused Video tests passed
Hardware-backed performance: diagnostic B/C measurements; full sign-off blocked by renderer parity/SIGSEGV
Software fallback: Vulkan llvmpipe, correctness only

## Build environment and diff inspection

WSL graphics preflight ran before WGPU tests. GL is accelerated through
Microsoft D3D12/NVIDIA; Vulkan is CPU llvmpipe. The diff contains only
performance instrumentation, adapter discovery, explicit WGPU script
selection, and this report. No hardware decode, YUV path, prefetch scheduler,
user feature, schema bump, or renderer rewrite was added.

## Evidence-based remaining performance opportunities

- Investigate the observed broad GL parity failures and SIGSEGV before a full
  hardware performance matrix.

## Remaining limitations

reverse playback; speed ramps/time remapping; frame interpolation; hardware
video decoding; YUV GPU path; Video audio auto-import; proxy media; full color
management.

## Scope confirmation

v1F Finalization was not started.

Subphase v1E Cross-Feature Integration & Performance is measured and documented.
Final sign-off remains blocked by the broad GL parity failures/SIGSEGV and the
pre-existing Python Ruff debt.
