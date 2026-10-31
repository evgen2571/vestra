# Shared video decoding across CPU workers
Status: completed
Branch / baseline: `perf/shared-video-decoding` / `34dec665090aa5706a83eb8302d38f5d1801dbf5`

## Objective
Reduce redundant native video decoding across CPU workers while preserving frame selection, output ordering, bounded memory, parallel rendering, and public APIs.

## Baseline and references
The baseline is `develop` at `34dec665090aa5706a83eb8302d38f5d1801dbf5`, including Phase 2 seeking and row-copy work. Relevant ownership and lifecycle guidance is in `docs/development/media-pipeline.md`, `docs/development/performance.md`, and the CPU backend/media modules listed in the assignment.

The audited path is `CpuBackend::submit_frame`/`poll_completed` in
`crates/vestra-render-cpu/src/cpu/backend.rs`, `run_worker` and
`CpuWorkerState::render_frame` in `cpu/worker.rs`, `PreparedAssets::video_source`
and `finish_frame` in `cpu/assets.rs`, and `VideoDecoder::frame_at` plus
`FfmpegVideoCursor::frame_at` in `vestra-media`. The SDK staged coordinator
reorders `CompletedFrame` values by frame number before encoding.

## Scope and non-goals
Scope is CPU worker scheduling, video frame reuse/coalescing, bounded memory, tests, measurements, and documentation. WGPU, schema, codec migration, general cache frameworks, and unrelated renderer rewrites are out of scope.

## Constraints and decisions
- Preserve existing timestamp-based selection and intelligent seeking.
- Keep independent source assets capable of parallel decoding.
- Any shared decoded data must be immutable and bounded.
- Prefer existing ownership and test seams; avoid new dependencies and public controls.
- Multi-worker baseline confirms round-robin workers duplicate native decoding (60 decodes with one worker versus 120 with eight on the existing fixture) even though wall time improves.
- A per-asset shared decoder prototype removed duplicate work but serialized same-source decoding and regressed the duplicate-source control (8-worker wall time 5.1 ms to 15.0 ms), so it was rejected.
- Use bounded temporal affinity instead: repeated-source plans queue at most four jobs per worker, rotating contiguous batches across workers. Existing per-worker cursors remain mutable and independent; unrelated assets keep parallelism. Plans without repeated video assets retain the original scheduler.

## Milestones
- [x] Audit decoding pipeline and record baseline measurements.
- [x] Evaluate temporally coherent scheduling and bounded shared decoding.
- [x] Implement the justified design with correctness and memory tests.
- [x] Run focused/full validation, document permanent behavior, commit, push, and open PR.

## Acceptance criteria
- Architecture and redundant work are evidenced with source references and counters.
- At least two approaches are compared and one is justified.
- CPU output remains equivalent across sequential, random, VFR/B-frame, same-source, multi-source, cancellation, and ordered-completion cases.
- Memory/backpressure behavior is bounded and measured where supported.
- Relevant Rust/Python/media/benchmark checks pass; unverified checks are named.
- Branch is pushed and a PR targets `develop` without merging.

## Progress and verification
- 2026-10-31: Recorded baseline branch/revision and read repository guidance; implementation audit completed.
- 2026-10-31: Duplicate-source control baseline/candidate (1/2/4/8 workers): 20.1/14.6/8.5/5.1 ms and 60/99/120/120 decodes versus 17.3/12.0/8.5/5.1 ms and 60/92/110/106 decodes. Single-layer control stayed on the ordinary path and measured 21.4/13.1/7.6/5.1 ms with 60/98/120/120 decodes on the candidate.
- 2026-10-31: Added a bounded queue/order regression test; all 182 non-ignored CPU tests passed. The shared-decoder prototype was discarded after its measured throughput regression.
- 2026-10-31: `cargo clippy -p vestra-render-cpu --all-targets -- -D warnings`, `cargo test -p vestra-media`, `cargo test -p vestra-render-common`, `cargo test -p vestra`, `just check`, and `just docs-check` passed. `just python-test` had five unrelated WGPU text-test failures because this environment returned no compatible WGPU adapter; 1,166 other Python tests passed.

## Decisions and discoveries

## Completion / handoff
The feature branch contains the temporal-affinity scheduler, bounded queue/order coverage, benchmark workload and permanent media-pipeline/benchmark documentation. Final commit, push, and PR metadata are recorded in the handoff report.
