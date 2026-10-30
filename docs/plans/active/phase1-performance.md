# Phase 1: sampling, cache threshold and native worker diagnostics
Status: in progress
Branch / baseline: perf/phase1-evidence / e19bd7f3f6c4b083d6cadc411714c5eba094365f

## Objective
Complete the three sequential experiments requested in the session, using reviewed Phase 0 evidence. Deliver a reviewable local diff and a reproducible report; a rejected candidate is valid.

## Baseline and references
The initial develop checkout is clean and exactly matches Phase 0's measured revision (not its directory suffix). Phase 0 report and supporting artifacts: ../../../../../benchmark-results/phase0-20261029T203158Z-81c53d/report.md (absolute location recorded in evidence).
Read README, CONTRIBUTING, PLANS, performance/benchmark guides and CPU architecture. Actual source/tests govern behavior.
Evidence: /home/evgen2571/drive/Development/vestra/target/benchmark-results/phase1-20261030T120042Z
Worktree: /home/evgen2571/drive/Development/vestra/target/worktrees/phase1

## Scope and non-goals
Repair scripts/benchmark_process.py and collector tests separately. A: only exact-center interpolation bypass in CPU raster, tests and controls. B: existing configured budgets 256/320 MiB; no default/policy change. C: test-only native thread bridge and fixed 1/2/4/8-worker diagnostic; no production concurrency API, decoder sharing or scheduler redesign. No GitHub changes, pushes, merges or publication.

## Constraints and decisions
Use equivalent installed native environment; copy Phase 0 build artifacts into new output before rebuilding, never write Phase 0. Record actual dependencies and refreshed binaries. Serial measurements, separated profiling, 1 warmup + 5 samples and repeated A/B blocks; expand if noisy. Preserve unavailable measurements and JSONL memory series. Pixel equivalence includes alpha-zero RGB normalization, crop/clipping, nonidentity colour, opacity/composition and Motion Tile fallback. Keep raw distributions and resource counters separate from wall timing. Do not sum nested/concurrent timers.

## Milestones
- [x] Sampler repair: failing regression, independent metrics/error context, focused collector tests.
- [x] Baseline/control preparation: source, native/tool/input hashes, smoke and fresh baseline.
- [x] A: eligibility proof, exact pixel/reference tests, source-sized/translation/negative/4K controls, canonical repeated timings, encoded correctness/resource checks; assess before B.
- [ ] B: SDK mixed10 cold/warm/warm-2 with 256/320 MiB, mixed25/50/fitting/static controls, exact full-frame checks and partitions/counters/memory/wall; assess before C.
- [ ] C: real native 1/2/4/8 workers, request/PTS/counter reconciliation and independent oracle, separate tracing/oracle and null/encoded performance/resource controls.
- [ ] Final checks/review/report and local handoff.

## Acceptance criteria
No semantic/API/default changes outside scope. Accept A only with exact correctness, repeatable intended-workload improvement, acceptable allocations/ownership and resolved canonical regressions. B must show real reuse and measured wall/memory tradeoff; no automatic production adoption. C succeeds on reliable attribution and correct pixels. Relevant Rust/Python/docs/encoded checks, fresh reviewer, and explicit unverified limits.

## Progress and verification
2026-10-30: read Phase 0 report; clean checkout equals measured source. Isolated worktree created. Copied build tree to new evidence directory to preserve native identity while allowing rebuilds. Sampler shared completeness bug confirmed in actual source. Sampler regression failed in all three expected cases, then collector suite passed 52/1 skipped. Separate sampler commit 731a7d0. CPU baseline suite: 175 passed/6 ignored. Fresh smoke suite passed ten scenarios. Canonical A1 is running serially; resource smoke successfully retained RSS/CPU despite FD AccessDenied. Native archive hashes match Phase 0 exactly; Rust/toolchain/locks unchanged.

## Decisions and discoveries
2026-10-30: User supplied reviewed design and requested autonomous routine decisions; proceed directly with this execution plan. Repository plan location/lifecycle takes precedence over generic skill scaffolding. No owning issue needed because user forbids issue modification and supplies full scope.

## Completion / handoff
Pending. Multi-hour output stability and publication beyond 4 GiB remain unverified.

2026-10-30 A progress: candidate eligibility regression failed first, then CPU suite passed 178/7 ignored, including exact RGBA filtered-reference comparisons at 720p/1080p/4K, partial/zero alpha, colour/opacity, crop, Motion Tile fallback, near identity and translations in both axes. Candidate commits 00b17e3 (candidate+correctness tests) and 02c9fb7 (test-only raster benchmark+plan). First canonical A/B pair completed; first-pair video medians fell approximately 39–48%, still provisional. All eleven first-pair CLI controls have byte-identical full decoded framemd5 sequences. Second A/B pair is running serially with original filtered loop restored for A2 (test-only eligibility helper retained under cfg(test)), then exact candidate source restored for B2. Each build/patch/binary/native identity is retained. Do not modify raster.rs while run-ab2.sh is running (session 80582); it owns temporary source restoration. Source-sized controls: 720/1080 identity, +1/-2 integer offsets; fractional, resize, opacity negatives; 4K correctness/resource only. Commands/drivers and complete resources JSONL live in evidence. No B or C implementation started.

2026-10-30 A decision: ADOPT narrow exact-center bypass, pending final contributor gates. Two full A/B pairs, two additional combined/nested pairs and raster-only A/B/A/B completed. Combined apparent +2.8/+7.6% regression did not repeat in focused pairs (+0.07/-9.09%); nested -0.89/-8.93%. All 33 control sequence comparisons, four composition sequences, 16 exact PNG checks and visual contact sheet passed. Six flat perf profiles succeeded, separately from timing. RSS/CPU valid and allocation/retained capacities match. See a-decision.json/a-analysis.md. Start B now; no production cache default change.
