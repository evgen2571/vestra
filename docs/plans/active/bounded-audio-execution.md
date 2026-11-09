# Bounded audio execution and consistent preparation
Status: paused
Branch / baseline: `feat/110-fast-audio-probing` / `4f7ff02` (PR #111)

## Objective
Address the follow-up review of [#111](https://github.com/evgen2571/vestra/pull/111)
and [#110](https://github.com/evgen2571/vestra/issues/110): bounded selections
must not incur a separate whole-source verification scan, metadata alone must
not warn, and one-shot/default/strict preparation must produce equivalent valid
output. User specification: attachment `48adc8e4-8c72-417d-ac58-2866078fb895`.

## Baseline and references
- Current published report: [audio probing](../../../benchmarks/audio-probing.md).
- Existing execution: FFmpeg graph shares sources, sample-trims/resets PTS,
  applies effects and padding; native full verification currently runs separately.
- Existing AAC regression shows 9 versus 10 frames at 8 fps when reported versus
  decoded omitted-end duration controls automatic output.
- See [media architecture](../../development/architecture/media-io.md),
  [timing](../../development/architecture/project-to-plan.md), and
  [output verification](../../development/media-pipeline.md#verify-encoded-output).

## Scope and non-goals
Media graph/source accounting, shared execution duration policy, provenance and
diagnostics, regression output, benchmark comparison to #109 and original #111,
documentation and existing PR/issue updates. No encoder replacement, global
cache, canonical project schema change, or silent weakening of strict validation.

## Constraints and decisions
- Count source samples after normalization inside the actual FFmpeg graph, using
  named `astats` Number_of_samples summaries. Reuse the existing stderr drainer;
  validate before publishing/accepting analysis. Missing counters fail closed.
- This count proves the consumed prefix only, never whole-source integrity or
  exact duration. Keep duration provenance unchanged by prefix verification.
- Keep logical clip selection/envelopes unchanged; compute required source
  prefixes from output-visible time and the ordered sample-duration transform.
  Read sequentially to preserve codec priming and source timestamp-gap semantics.
- One-shot/default preparation share exact EOF resolution for omitted trim ends;
  strict preparation additionally audits all sources. Bounded execution can use
  reported or unavailable metadata without scanning merely to check bounds.
- Exact EOF-dependent automatic timelines fundamentally require reaching EOF.
  Common reported timelines break AAC/FLAC semantics; late recompilation breaks
  prepared snapshot frame-count/resource contracts. Preserve exact end planning.
- Remove ordinary reported-duration warnings; retain inspection provenance.
- Test FFmpeg summary parsing and real Windows/macOS CI, since stderr summaries
  are an implementation contract rather than a formal machine-readable API.

## Milestones
- [x] Investigate actual read pipeline, duration consistency and alternatives.
- [x] Integrated bounded counting, failures and analysis/output regressions.
- [x] Shared exact-end preparation policy and warning removal across surfaces.
- [ ] Benchmarks: #109, original #111, revised code; first/repeated/one-shot,
  preparation-plus-render totals, RSS, strict/exact behavior and published media.
- [ ] Relevant local checks and final GitHub Actions including Windows/macOS.
- [ ] Focused commits, updated report/PR description and accurate issue boxes.

## Acceptance criteria
Use all five sections of the user specification. Preserve cancellation, destination
cleanup, source-bound diagnostics, source sharing, timestamp normalization, fades,
speed, AAC padding, misleading/truncated/corrupt media and exact-duration limits.
Verify real decoded samples/PTS and report consumed-region versus full audit scope.

## Progress and verification
Original worktree remains clean on develop; PR worktree started clean at 4f7ff02.
Original PR macOS and Windows native checks passed; revised checks pending.
One-pass counters stop at a prefix and expose EOF hidden by padding. Focused
checks passed: core audio 8, SDK validation 21, editor 7, graph 32, native output
integration 9, media suite 136 (5 ignored), plus a new analysis EOF/prefix test.
Full `just check`, forced Python rebuild/test (1,435 passed / 42 skipped),
Python lint and strict site checks passed in `target/bounded-audio-check.log`.
Final color-environment hardening passed all nine native output tests and the
analysis bounds regression. Three-version benchmark timing finished: 81 workers,
one warmup plus five samples; independent full decoding verified all 62 outputs.
Raw data is under `target/benchmark-results/audio-bounded-execution/`; report
refresh is in progress. Linux Rust/Python/integration CI passed at 90644a8;
Windows and macOS fail the same AAC sample-count discrepancy (below).

## Decisions/discoveries
- Metadata duration can be absent even for a bounded selection; execution must
  avoid the inspection fallback scan and compile explicit ends without inventing
  an asset duration.
- Bounds beyond reported metadata are provisional during bounded execution;
  strict preflight still validates the complete authored selection.
- Strict FFmpeg error handling exposed final-master padding with invalid PTS in
  the existing AAC crossfade test. Reproduced independently, then fixed by sample-
  based timestamp normalization after final padding (red→green). All media tests
  and native output regressions pass with the stricter error handling.
- The existing per-path reuse test's scoped tracing callsite could be cached as
  disabled by parallel tests. A second live test-only dispatch stabilizes tracing;
  its exact one-probe assertion and parallel suite remain intact.

## Completion / handoff
Paused at the user's request on 2026-11-09. Preserve this plan for future work.
PR #111 is to remain open as a draft, targeting develop; do not merge.

Current snapshot includes the transformed-output EOF guard, a same-reader exact
end resolver with SDK wiring, adaptive AAC integration expectations, and the
three-version benchmark report. These are work in progress, not a completed PR.

Verified before pausing:
- Speed-coverage regression: 139 media tests passed / 5 ignored, and the SDK
  destination-preservation regression passed (before the new reader helper).
- New exact-reader helper: four focused tests passed with external FFmpeg 8.1.2
  and four passed with a diagnostic FFmpeg 9.0.2 build, including cancellation.
- Formatting, Python lint and documentation source links passed.
- All 81 benchmark workers and independent decoding of 62 outputs completed for
  90644a8; the report explicitly identifies that measured revision.

Still required before review-ready status:
- Run the SDK/CLI integration and full relevant local gates after the reader
  change, including fresh Python native builds/tests.
- Verify final Windows/macOS CI. At 90644a8 both fail the AAC 60,416-versus-60,000
  mismatch; the new resolver has not yet been validated by those jobs.
- Refresh the revised benchmark executable, timings and decoded output checks
  after the production changes; audit report/PR/issue acceptance evidence.
- Review/finalize the exact-reader implementation and contracts; keep sources
  and the chosen FFmpeg executable unchanged for prepared snapshots.

- 2026-11-09: Native Windows and macOS external FFmpeg 9.0.2 readers produce
  60,000 samples for AAC whose linked FFmpeg 8.1.3 scan counts 60,416. Actual
  coverage correctly rejects the disagreement. Investigate codec end padding
  and edit-list behavior; do not weaken bounds or pin old external FFmpeg.
  Both platforms passed the other eight native integration tests.

- 2026-11-09: User explicitly selected same-reader exact duration resolution:
  retain native fast metadata/inspection, use installed rendering FFmpeg for
  exact/strict preparation. Native-only exception authorized after deterministic
  same-file FFmpeg8/9 60,416-versus-60,000-sample reproduction. Inspection can
  retain native decoded provenance; rendering uses execution-reader semantics.
- Review found atempo EOF flushing can fall short of ideal inverse-duration
  coverage. Added transformed pre-padding counters for incomplete selections;
  complete authored selections retain normalization. Red→green media/SDK tests.
