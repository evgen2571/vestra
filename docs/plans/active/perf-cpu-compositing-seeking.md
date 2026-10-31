# Phase 2 CPU compositing and intelligent seeking
Status: completed
Branch / baseline: `perf/cpu-compositing-seeking` / `develop` `8b9f737`

## Objective
Add a semantics-preserving CPU row-copy path for eligible opaque integer-translated raster layers and adaptive FFmpeg repositioning for distant frame requests, with focused correctness tests and benchmark evidence.

## Baseline and references
- CPU composition entry point: `crates/vestra-render-cpu/src/cpu/compositor.rs`.
- CPU raster sampling: `crates/vestra-render-cpu/src/cpu/raster.rs`.
- Native decoder policy: `crates/vestra-media/src/video.rs` and `video_cursor.rs`.
- Existing Phase 1 exact-center sampling: commit `ca93653`.
- Baseline decoder sought only when a request preceded the current presentation frame; forward requests decoded sequentially.

## Scope and non-goals
- Change only CPU raster/composition and native decoder seek policy, plus focused tests, concise docs and benchmark reporting.
- Preserve public APIs, project format, WGPU behavior and decoder fallback semantics.
- No shared decoder, new keyframe index, cache redesign or unrelated renderer cleanup.

## Constraints and decisions
- Row copy requires normal source-over, unit opacity, no effects/masks/mattes, default colour transform, full uncropped source, unit scale, zero rotation and an integer pixel translation. Source alpha must be proven fully opaque; all other cases use the existing sampler/compositor.
- Seek policy uses timestamp distance and metadata-derived frame estimates only as a cost heuristic. FFmpeg still seeks to a preceding random-access point and the existing PTS selection loop remains authoritative.
- A backward seek failure returns the existing media error; failed initial/forward repositioning falls back to sequential decoding, preserving frame selection.

## Milestones
- [x] Implement and test row-copy eligibility, clipping, offsets and fallback.
- [x] Implement and test forward/initial adaptive seeking across sequential, backward, VFR and long-GOP fixtures.
- [x] Run focused validation and capture isolated benchmark results.
- [x] Update permanent documentation and complete this plan before handoff.

## Acceptance criteria
- Exact pixel equality with the existing path for eligible and ineligible layer cases.
- No public API or WGPU behavior changes.
- Sequential access avoids unnecessary seeks; distant initial/forward requests reduce decoded frames where FFmpeg supports seeking.
- Tests cover transparency, clipping, transforms, repeated/backward/VFR/EOF access and memory bounds.
- Benchmark commands and environment are recorded; unavailable checks are explicitly marked unverified.

## Progress and verification
- Baseline branch and SHA recorded above; repository clean before branch creation.
- Opacity proofs are computed once for prepared images/shapes/text and once per decoded video frame, so transparent fallback work does not rescan the source in the compositor.

## Decisions and discoveries
- 2026-10-31: Phase 1 already has a full-frame cached opaque copy; Phase 2 adds partial row copy at raster presentation and adaptive seek decisions at the decoder boundary.

## Completion / handoff
Pending implementation and validation.

### 2026-10-31 progress
- Row-copy implementation committed as `2c198b7`; focused clipped opaque and transparent/effect fallback tests pass.
- Adaptive seeking committed as `55024b6`; media suite passes 113 tests plus 4 ignored, including long-GOP/B-frame, VFR, EOF, backward and distant-forward cases.
- CPU unit suite passes 180 tests plus 11 ignored.
- Opacity eligibility is cached in prepared raster sources after review; the final CPU suite passes 180 tests plus 12 ignored, including the added row-copy benchmark.
- Canonical smoke baseline/candidate captures completed under the pinned Nix shell at 128x72 CPU settings. The suite's existing workloads did not exercise the new row-copy eligibility (both reports show zero opaque-copy hits), and ordinary sequential workloads remained within run-to-run noise. Production-edit smoke remained effectively unchanged (baseline 1289 ms, combined 1286 ms; one sample each).
- Release row-copy microbenchmark (`opaque_row_copy_benchmark`) on 1280x720, eight frames per sample, five measured samples: filtered median 486.57 ms versus row-copy median 6.39 ms (98.69% elapsed reduction for this fully eligible workload). This is a focused path measurement, not a whole-render claim.
- Release native decoder matrix (1280x720 H.264, 180 frames, GOP 60, B-frames 3) compared baseline `8b9f737` and combined code. Sequential remained 180 decodes/0 seeks; scrub at zero cache improved from median 4,785,250 us / 2,436 decodes / 19 seeks to 4,195,170 us / 2,084 decodes / 23 seeks. Sparse-forward requests in that six-second fixture stayed below the four-second threshold and were unchanged. Pixel oracles passed in both runs.

## Completion / handoff
Implementation and focused validation are complete. Full workspace checks, Python/API checks, WGPU software/hardware checks, and cross-platform runs remain unverified in this environment. The feature branch is intentionally unpushed and no merge was performed.
