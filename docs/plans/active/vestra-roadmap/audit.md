# Backlog audit evidence

Baseline: `develop` at `4c77b44431c355322c5fcaf0481bf02fa0e18356`, 2026-10-16.
This audit changed no production code. Source-confirmed facts are distinguished
from executed reproductions and unverified hypotheses.

## Architecture and existing contracts

Read README, CONTRIBUTING, AGENTS, issue-tracker/domain/label configuration,
PLANS, crate overview/ownership, project-to-plan, CPU/WGPU architecture, effect
pipeline/integration, Python bindings, media pipeline, performance, testing,
project-format and backend support material. No glossary, ADR or prior rejection
record exists in this baseline. CodeGraph is available but the repo is unindexed.

Core owns the canonical model, validation, checked timeline math, compilation
and evaluation. SDK preflight supplies media facts. Renderers consume evaluated
work. Media owns probing, decode, encoding and publication. Python authoring
lowers to the same SDK. The proposed specs preserve these boundaries.

## Tracker inventory and duplicate check

All issues including closed issues, bodies and comments were fetched with `gh`.
No open issues exist. The one closed issue is
[#4: WGPU additive particle staging can exceed requested max_buffer_size](https://github.com/evgen2571/vestra/issues/4).
It has no comments. It is historical resource-limit context, not evidence that
the bug remains present, and it will not be reopened or duplicated.

Native sub-issue and blocked-by GET endpoints on #4 both return `[]`. Actual
relationship creation still requires write access and read-back verification.

## Timeline findings

### Confirmed by executing unchanged core validation

A temporary Rust executable linked the current compiled `vestra-core` and
`serde_json`. Its assert-based cases all completed successfully:

| Fixture | Actual result | Ownership |
| --- | --- | --- |
| Explicit 7201 seconds, visual clip 7201 seconds, 30 FPS | Two `VESTRA-LIMIT-TIMELINE` errors: duration and frame count | T01 |
| Explicit 7201 seconds, visual clip one second | Pure validation accepts; resolved preflight has a separate final-budget check | T03 |
| Explicit one second, visual clip 7201 seconds | Pure validation rejects before effective-output clipping | T03 |
| Explicit ten seconds, visual clip one second, global brightness keyframe at five seconds | `VESTRA-KEYFRAME-TIME` at global amount keyframe time | T02 |

The two-second dimension is 2×2, no assets/media are required, and the amount
value is within its legal range. These are validation reproductions, not
multi-hour encoding results. The first case demonstrates existing defaults;
it is an agreed policy change rather than an accidental implementation bug.

Source authority:

- `ResourceLimits::default()` sets `maximum_frames = 216_000` and
  `maximum_duration_seconds = 7_200.0`.
- Core validation's `visual_duration()` uses clip/flash ends irrespective of
  explicit output duration; it supplies both global-track timing and the first
  timeline-budget check.
- SDK `duration::resolve` implements explicit output clipping and automatic
  max(visual, flash, resolved audio). Existing tests deliberately preserve
  structural duration even with muted/zero-gain audio or audio output disabled.
- SDK preflight already invokes `enforce_final_timeline_limits` after media
  duration resolution. This protection must be retained, not recreated.

### Confirmed by source inspection, not runtime reproduction

- High-level `Composition.group` requires or inherits an explicit containing
  duration. `Composition.add` infers video duration from media, but other omitted
  durations copy the containing span; start is not subtracted. These are current
  contracts, not automatically bugs. T05/T06 investigate the proposed changes.
- Canonical `DurationMode` has Automatic and Explicit only. Minimum mode is a
  proposed addition (T07), not an established decision.
- Recursive `validate_video_layers` discards the ancestor diagnostic path and
  constructs `/visual/clips/{index}` in each recursion (T09). A runtime nested
  regression must precede the fix.
- Video source-end checks use authored source_start + duration × playback_rate;
  exact EOF holding belongs to decoder frame selection. T08 investigates their
  interactions with effective clipping rather than declaring EOF holding absent.
- Transition endpoint validation compares floating-point interval ends directly;
  no incorrect transition rejection is reproduced. T04 owns this hypothesis.
- Checked rational arithmetic, half-open activity, ceiling frame counts,
  timestamp mapping and fractional-rate random-access tests already exist.
  T10 expands long-range coverage, not a wholesale timeline rewrite.

Primary inspected source: core validation/output/limits/tracks/effects and
timeline modules; SDK project validation/duration/preflight and prepared-frame
entry points; Python editor, lowering, builder and high-level group/video tests.

## Performance and resource findings

- Versioned benchmark suites already capture raw repeated samples, source and
  environment identity, actual adapter/backend, workload compatibility and
  stage/resource reports. P01 captures real baselines; it does not rebuild this.
- Reported resource bytes are engine counters/estimates rather than process RSS.
  P02 adds measured process/child memory with explicit measurement scope.
- Preparation, frame evaluation, render, encoding, publication and WGPU
  upload/submit/readback timers exist. CPU has opt-in nanosecond hot-path reports.
  P03 audits missing cost attribution before adding instrumentation.
- WGPU `submit_frame` computes `has_groups` and filters out every static-layer
  candidate if true. This is a source-confirmed opportunity, not a proven speedup.
  P04 requires a cache-disabled pixel reference and paired measurements.
- WGPU retains topology but `GpuFramePlan::build_inner` allocates operation/layer
  vectors per frame. Reuse feasibility is P05, not an assumed harmless refactor.
- CPU already retains composition/effect scratch, caches complete static groups,
  special-cases static plans, and restricts Gaussian work to alpha bounds.
- Core already normalizes constant/identity tracks and effects and fuses a
  compatible static colour chain. Broader fusion, ROI, SIMD and scheduling
  benefits remain hypotheses.
- Byte LRU caches, worker capacity, staging/readback slots, pending static-cache
  reservations and GPU completion lifetimes already exist. Their bounds are not
  a measured aggregate whole-process memory guarantee.

Primary inspected source: benchmark runner and animation_effects benchmark;
CPU backend and work-elimination benchmarks; WGPU backend/frame plan/topology/
texture pool; core compiler optimization/dependency/limits; SDK stage metrics.

## Media findings

- Encoder defaults are libx264/yuv420p MP4. Quality selects CRF and internal
  x264 preset: preview/30 → ultrafast, balanced/23 → veryfast, high/18 → medium.
  Independent public encoder controls are missing; exact new controls are undecided.
- Video render requests lack selected-range fields, while prepared single-frame
  number/timestamp access already exists. Range semantics belong to M08.
- PTS-based source selection, VFR/B-frame handling, EOF hold and bounded RGBA
  caching already exist. CPU reuses primary per-asset decoders; concurrent offsets
  use extra cursors. WGPU uses bounded native prefetch. M05 targets remaining
  scalability costs and includes many assets/placements/file descriptors.
- Dynamic WGPU already overlaps one ordered encoder write with a subsequent
  submission; this is neither absent nor an unbounded queue. M06 evaluates tuning.
- Master PCM is streamed in chunks with bounded stderr. Do not invent a whole-PCM
  retention problem; retained signal/analysis growth still needs measurement.
- Native audio-source limits and large filtergraph temporary-file handling
  already exist, with failure/cancellation cleanup tests.
- Temporary output finalization/publication and substantial sink error tests
  already exist. Multi-hour/large-file timestamp behavior and hardware paths were
  not executed in this audit.

Primary inspected source: core EncoderSettings/output model, media FFmpeg sink,
audio PCM, video cursor/cache and sink tests, SDK staged/static frame loops,
prepared video request and public SDK tests.

## Architecture, quality and DX findings

Canonical visual/audio effect descriptors, backend-neutral pass planning and
Python SourceCapabilities are established prior art. Working GPU textures are
encoded straight-alpha Rgba8Unorm; some private accumulations are premultiplied.
This is not established linear-light, high-precision or HDR processing.

The feature support matrix distinguishes verified stylization coverage from
effects/transitions with incomplete visual verification. Quality tickets extend
gaps, retain alpha/error semantics and keep software/hardware claims separate.

Inspection exposes resolved output/audio and visual counts, but not a full
resolved nested visual timeline. Adapter metadata, selection/fallback results and
descriptors exist, but need coherent high-level discovery. ResourceLimits exists
internally while ordinary public Editor validation selects defaults. Public
configuration shape and editing helpers remain undecided.

## Checks performed and limits

- Four focused core timeline tests passed, including fractional timestamp
  self-consistency through 10000 frames per tested rate.
- Full `cargo test --offline -p vestra-core`: 327 unit tests and one integration
  test passed; one manual test ignored.
- Temporary validation reproducer: all four assertions passed with the results
  above. The executable/source live under `/tmp`; no production regression test
  or feature fix was added by this planning task.
- No CPU/hardware performance baseline, hardware frame render, damaged-media
  run, multi-hour output or Python-native/full-workspace suite was executed.
  Those remain explicit verification requirements of their future tickets.
