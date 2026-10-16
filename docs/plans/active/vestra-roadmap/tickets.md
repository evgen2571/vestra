# Vestra roadmap: proposed ticket breakdown

Status: historical pre-publication ticket draft. Publication and verification completed.

The original breakdown below is preserved, including its superseded P0 priorities
and review prompts. It is not a second tracker. See [publication status and issue map](README.md)
for approved changes and the authoritative GitHub issues. Do not update implementation progress here.
Baseline: `develop` at `4c77b44431c355322c5fcaf0481bf02fa0e18356` (2026-10-16).

This file is a review artifact, not a second issue tracker. GitHub Issues remains authoritative after publication.

## Review decisions

- Confirm ticket granularity, genuine blockers, and whether to merge/split anything.
- Approve six specs plus one master roadmap and these 50 child tickets for publication.
- Priority is recorded as `[P0]`–`[P3]` in titles and in bodies; no new label taxonomy.
- Every child uses one category (`bug` or `enhancement`) and one triage state (`ready-for-agent`). Research/prototype/measurement tickets are ready for that bounded investigation only, never for adopting an undecided design.
- Specs cover agreed objectives and research boundaries. They do not authorize implementation of undecided APIs. The roadmap is a coordination container (`enhancement`, `ready-for-human`).
- Create native sub-issue hierarchy roadmap → specs → tickets, and native blocking links when available; retain explicit parent references/fallback links if native operations are unavailable.
- All published issue bodies/comments start with the configured AI triage disclaimer. Existing closed issue #4 is historical context, not reopened or duplicated.
- Preserve existing parent bodies: create tickets before publishing each new spec with its final links, then create the master with final links; attach native relationships afterward. Dependency creation orders blockers before dependents.

## Counts

| Workstream | P0 | P1 | P2 | P3 | Total |
| --- | --- | --- | --- | --- | --- |
| Timeline and long-form correctness | 10 | 0 | 0 | 0 | 10 |
| Performance measurement and optimization | 0 | 4 | 5 | 2 | 11 |
| Media pipeline and encoding | 0 | 4 | 3 | 3 | 10 |
| Architecture and maintainability | 0 | 3 | 3 | 0 | 6 |
| Correctness and quality | 0 | 5 | 2 | 1 | 8 |
| Python API, tooling and developer experience | 0 | 3 | 2 | 0 | 5 |
| Total | 10 | 19 | 15 | 6 | 50 |

## Ready-to-start recommendation

Implementation/test frontier: T01, T02, T03, T09, T10; in parallel P02, P03, A01, A05, Q01, Q03, Q04, Q05, D01 and D04. P01 establishes measurement evidence in parallel. Prioritize T02/T03 as compact reproduced fixes, with T01 the main long-form unlock. These tickets share some code, so assign ownership before concurrent edits; independence means no semantic blocking edge, not conflict-free file edits.

No spec, benchmark capture or umbrella issue blocks unrelated fixes. Every unblocked research ticket can also start; implementation of its proposed approach needs a later reviewed decision.

## Numbered breakdown for approval

1. **T01 [P0] Remove arbitrary default timeline caps** (implementation). Blocked by: None. Delivers: Long timelines validate and prepare without opting out of arbitrary defaults, while explicit caller budgets and representability checks remain enforced.

2. **T02 [P0] Validate global effect timing against the resolved project span** (bug). Blocked by: None. Delivers: Global effects use project-local resolved duration consistently across validation, preparation and rendering.

3. **T03 [P0] Apply timeline budgets to the effective output duration** (bug). Blocked by: None. Delivers: Custom timeline budgets constrain the effective output timeline consistently in pure validation and resolved preflight.

4. **T04 [P0] Investigate transition and clip boundary inconsistencies** (investigation). Blocked by: None. Delivers: A minimal case matrix distinguishes valid end-boundary behavior from floating-point or time-domain defects.

5. **T05 [P0] Define nested automatic duration inference** (investigation). Blocked by: None. Delivers: A reviewed contract for inferring group placement duration from child-local content without changing existing explicit placements.

6. **T06 [P0] Define fill-to-project-end layer timing** (investigation). Blocked by: None. Delivers: A reviewed contract for fill-to-end layers that avoids circular automatic-duration inference.

7. **T07 [P0] Assess a minimum-duration mode** (investigation). Blocked by: None. Delivers: An accept/defer recommendation for a minimum timeline duration with a precise candidate contract.

8. **T08 [P0] Reconcile source-end checks with clipped execution** (investigation). Blocked by: None. Delivers: A source-time boundary matrix and recommendation covering visible execution versus authored source validity.

9. **T09 [P0] Preserve nested media diagnostic pointers** (bug). Blocked by: None. Delivers: An invalid nested video source span reports its actual canonical nested location and owning clip.

10. **T10 [P0] Extend long-range rational-FPS boundary coverage** (verification). Blocked by: None. Delivers: A compact regression matrix verifies frame counts and random access far beyond the former default limits.

11. **P01 [P1] Capture reproducible CPU and hardware-WGPU baselines** (measurement). Blocked by: None. Delivers: Comparable baseline artifacts identify CPU, real hardware WGPU, software correctness and encoded versus frame-only costs separately.

12. **P02 [P1] Measure process memory alongside engine counters** (implementation). Blocked by: None. Delivers: Benchmark records distinguish measured process memory from estimated engine/GPU resource counters.

13. **P03 [P1] Expose actionable preparation and frame-work profiles** (implementation). Blocked by: None. Delivers: Users can isolate the remaining unaccounted evaluation, plan-build, allocation and media costs without conflating overlapping stages.

14. **P04 [P1] Prototype WGPU static reuse in scenes containing groups** (prototype). Blocked by: None. Delivers: A minimal paired prototype tests whether safe caching can benefit nested scenes.

15. **P05 [P2] Prototype frame-plan reuse for stable topology** (prototype). Blocked by: P03. Delivers: A measured prototype reuses only invariant plan work while preserving all frame-dependent values.

16. **P06 [P2] Reduce measured frame and buffer allocation hotspots** (prototype). Blocked by: P03. Delivers: One measured allocation hotspot is reduced with owned returned frames and bounded scratch.

17. **P07 [P2] Evaluate backend choice and worker scheduling tradeoffs** (investigation). Blocked by: P01, P02. Delivers: Evidence-based recommendations explain which workload/worker configurations improve throughput within memory limits.

18. **P08 [P2] Assess additional effect-pass fusion** (prototype). Blocked by: P03. Delivers: One representative unfused chain has a correctness-checked cost assessment and optional minimal prototype.

19. **P09 [P3] Assess ROI and tiled execution for a bounded workload** (prototype). Blocked by: None. Delivers: A bounded experiment identifies whether one sparse workload benefits from extending spatial work reduction.

20. **P10 [P3] Assess SIMD for one measured CPU kernel** (prototype). Blocked by: None. Delivers: One portable hotspot has a measured SIMD feasibility result.

21. **P11 [P2] Assess aggregate memory budgets and transient lifetimes** (investigation). Blocked by: P02. Delivers: A resource inventory and stress measurements expose peak overlap and recommend the smallest budget/lifetime improvement.

22. **M01 [P1] Define public encoder configurability** (investigation). Blocked by: None. Delivers: A reviewed minimal output-encoding contract builds on existing quality defaults.

23. **M02 [P2] Assess hardware encoding capability and fallback** (prototype). Blocked by: None. Delivers: A device-specific feasibility report and minimal encode prototype compare hardware and software output.

24. **M03 [P2] Assess hardware video decode on a representative source** (prototype). Blocked by: None. Delivers: A focused hardware-decode experiment measures decode/conversion costs without changing source-time semantics.

25. **M04 [P3] Assess GPU-to-encoder transfer paths** (investigation). Blocked by: M02. Delivers: A feasibility result describes one realistic hardware-encoder interop path and its end-to-end costs.

26. **M05 [P1] Profile decoder reuse and source scalability** (investigation). Blocked by: None. Delivers: Evidence identifies remaining decoder churn or excessive retention across many placements and assets.

27. **M06 [P2] Assess render/encode overlap under sustained load** (prototype). Blocked by: P01, P02. Delivers: A paired experiment determines whether tuning existing overlap improves a representative bottleneck.

28. **M07 [P1] Validate multi-hour muxing, large files and AV sync** (verification). Blocked by: T01. Delivers: A gated media validation workload detects long-run timestamp, container and AV drift failures.

29. **M08 [P1] Define selected-range video rendering** (investigation). Blocked by: None. Delivers: A reviewed range-render contract specifies output time, source time, audio and analysis semantics.

30. **M09 [P3] Design segmented and resumable rendering** (investigation). Blocked by: M08. Delivers: A design/prototype decision assesses safe resumability using approved range semantics.

31. **M10 [P3] Assess additional containers and codecs** (investigation). Blocked by: None. Delivers: A prioritized capability matrix recommends a first additional output combination or defers it.

32. **A01 [P1] Guard shared capability declarations against drift** (verification). Blocked by: None. Delivers: Contract tests detect disagreement between canonical descriptors, public schema, authoring and backend support claims.

33. **A02 [P2] Simplify one effect integration path** (investigation). Blocked by: None. Delivers: A concrete integration map identifies redundant declarations and proposes one small simplification.

34. **A03 [P1] Consolidate resource-policy ownership** (investigation). Blocked by: None. Delivers: A reviewed policy map distinguishes canonical constraints, caller budgets and device/execution limits.

35. **A04 [P2] Identify a justified rendering or compiler module split** (investigation). Blocked by: None. Delivers: One bounded refactor proposal isolates a measured maintenance risk and names protected behavior.

36. **A05 [P1] Strengthen prepared-resource ownership and teardown contracts** (verification). Blocked by: None. Delivers: Public lifecycle regressions protect frame ownership, busy state, teardown and failure cleanup.

37. **A06 [P2] Assess render-plan abstraction limits from concrete workloads** (investigation). Blocked by: None. Delivers: A constraint analysis determines whether any planned workload needs a minimal IR extension.

38. **Q01 [P1] Expand fractional-FPS and timestamped media regressions** (verification). Blocked by: None. Delivers: A compact media matrix protects source selection and audio alignment at fractional output rates.

39. **Q02 [P1] Stress bounded resources on long and repeated renders** (verification). Blocked by: T01. Delivers: Gated stress workloads reveal duration-dependent retention and repeated-operation leaks.

40. **Q03 [P1] Expand temporal CPU and WGPU visual parity coverage** (verification). Blocked by: None. Delivers: Sequence-based regressions cover currently under-verified temporal interactions.

41. **Q04 [P1] Expand corrupt-media and failure-recovery tests** (verification). Blocked by: None. Delivers: Representative damaged media and mid-operation failures produce actionable errors and leave valid destinations intact.

42. **Q05 [P1] Protect alpha semantics across composition stages** (verification). Blocked by: None. Delivers: A focused cross-backend fixture matrix catches hidden-RGB and edge contamination.

43. **Q06 [P2] Define explicit colour-management requirements** (investigation). Blocked by: None. Delivers: A reviewed colour contract identifies user needs and a compatibility path.

44. **Q07 [P3] Assess higher-precision and HDR processing** (prototype). Blocked by: Q06. Delivers: A bounded prototype evaluates one higher-precision path against approved colour requirements.

45. **Q08 [P2] Assess resampling and preview-versus-final quality** (prototype). Blocked by: None. Delivers: A visual/cost comparison recommends one narrowly scoped quality improvement.

46. **D01 [P1] Expose resolved visual timeline inspection** (implementation). Blocked by: None. Delivers: Users inspect local/project visual intervals and resolved output timing through the shared SDK, Python and CLI.

47. **D02 [P1] Define public resource-limit configuration** (investigation). Blocked by: A03. Delivers: A reviewed configuration contract lets callers set finite budgets consistently across validation, preflight and preparation.

48. **D03 [P2] Assess a minimal timeline manipulation API** (prototype). Blocked by: None. Delivers: A small prototype validates the most useful timeline-edit operation without introducing an editor framework.

49. **D04 [P1] Expose backend capabilities and selection reasons** (implementation). Blocked by: None. Delivers: Callers discover actual backend/adapter availability and supported operations before an expensive render.

50. **D05 [P2] Make performance reports useful for render diagnosis** (implementation). Blocked by: P02, P03. Delivers: Users can inspect a coherent performance report without parsing unrelated tracing output.

## Timeline and long-form correctness

Spec draft: [spec-timeline.md](spec-timeline.md).

### T01: [P0] Remove arbitrary default timeline caps

**Type:** implementation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** ResourceLimits defaults reject more than 216000 frames or 7200 seconds; reproduced with a 7201-second visual clip.

**What it delivers:** Long timelines validate and prepare without opting out of arbitrary defaults, while explicit caller budgets and representability checks remain enforced.

**Acceptance criteria:**

- [ ] Accept more than 216000 frames and more than 7200 seconds with defaults without rendering every frame.
- [ ] reject overflow, nonfinite timing and deliberately small custom budgets.
- [ ] preserve dimension, decoded-memory, cache, complexity and active-layer protections.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T02: [P0] Validate global effect timing against the resolved project span

**Type:** bug. **Category/state:** bug / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Reproduced: an explicit 10-second project with one second of visual content rejects a global brightness keyframe at five seconds.

**What it delivers:** Global effects use project-local resolved duration consistently across validation, preparation and rendering.

**Acceptance criteria:**

- [ ] Accept the five-second keyframe in the explicit ten-second project.
- [ ] cover audio-extended automatic and empty-visual explicit projects.
- [ ] reject genuinely out-of-span or malformed tracks without requiring I/O during pure validation.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T03: [P0] Apply timeline budgets to the effective output duration

**Type:** bug. **Category/state:** bug / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Reproduced: pure validation rejects a one-second explicit output containing a 7201-second clipped clip, yet accepts a 7201-second explicit output containing a one-second clip.

**What it delivers:** Custom timeline budgets constrain the effective output timeline consistently in pure validation and resolved preflight.

**Acceptance criteria:**

- [ ] Test short explicit output with long clipped content and the converse using finite custom budgets.
- [ ] retain media-resolved audio enforcement and truncation warnings.
- [ ] do not let invalid authored times bypass validation.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T04: [P0] Investigate transition and clip boundary inconsistencies

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Transitions compare floating-point placement ends directly to endpoint lifetimes; core rendering uses checked half-open frame intervals. No transition failure has been reproduced.

**What it delivers:** A minimal case matrix distinguishes valid end-boundary behavior from floating-point or time-domain defects.

**Acceptance criteria:**

- [ ] Cover coincident clip ends/starts, nested local time, transformed playback and rational FPS.
- [ ] retain a reproducer for every confirmed defect.
- [ ] recommend the smallest correction and explicitly close non-reproducing hypotheses without speculative tolerance changes.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T05: [P0] Define nested automatic duration inference

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** High-level group creation requires a duration or inherits the containing explicit span; child-content inference is missing.

**What it delivers:** A reviewed contract for inferring group placement duration from child-local content without changing existing explicit placements.

**Acceptance criteria:**

- [ ] Compare nested offsets, empty groups, media-derived children and post-effects.
- [ ] resolve whether omitted duration means inference or inherited span and document migration implications.
- [ ] propose a bounded implementation slice only after the behavior is approved.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T06: [P0] Define fill-to-project-end layer timing

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Omitted non-video layer duration copies the entire containing duration even when start is positive; no persistent fill-to-end intent is represented.

**What it delivers:** A reviewed contract for fill-to-end layers that avoids circular automatic-duration inference.

**Acceptance criteria:**

- [ ] Compare remaining-span versus inherited-span behavior with nonzero starts.
- [ ] specify treatment of empty projects, audio-extended timelines, nested placements and explicit overruns.
- [ ] preserve existing explicit numeric durations and identify compatibility impact.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T07: [P0] Assess a minimum-duration mode

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Canonical DurationMode supports only automatic and explicit; minimum-duration behavior is a proposal.

**What it delivers:** An accept/defer recommendation for a minimum timeline duration with a precise candidate contract.

**Acceptance criteria:**

- [ ] Compare explicit clipping and automatic max-content behavior with max(minimum, resolved content).
- [ ] cover empty content, media probing and invalid minimum values.
- [ ] require maintainer approval before adding schema or API values.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T08: [P0] Reconcile source-end checks with clipped execution

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Preflight checks the full authored video source span while the decoder can hold its final frame; these apply at different boundaries. Effective clipping interactions are unverified.

**What it delivers:** A source-time boundary matrix and recommendation covering visible execution versus authored source validity.

**Acceptance criteria:**

- [ ] Exercise source_start, playback rate, exact EOF, VFR/B-frame tails, explicit project clipping and nested placements.
- [ ] distinguish intentional rejection from reproducible false errors.
- [ ] specify diagnostics and compatibility before changing EOF or validation policy.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T09: [P0] Preserve nested media diagnostic pointers

**Type:** bug. **Category/state:** bug / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Source inspection confirms recursive video preflight restarts diagnostic pointers at /visual/clips for every group level.

**What it delivers:** An invalid nested video source span reports its actual canonical nested location and owning clip.

**Acceptance criteria:**

- [ ] Add a failing nested-media diagnostic regression before fixing it.
- [ ] cover at least two nesting levels and distinguish sibling indices.
- [ ] retain diagnostic code and meaningful clip identity without changing source-end policy.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### T10: [P0] Extend long-range rational-FPS boundary coverage

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Checked integer frame arithmetic and fractional-rate prepared-frame tests already exist; the focused four-test timeline suite passes.

**What it delivers:** A compact regression matrix verifies frame counts and random access far beyond the former default limits.

**Acceptance criteria:**

- [ ] Cover 24000/1001, 30000/1001 and 60000/1001 with independent expected integer results.
- [ ] test first/last frame, exact exclusive end and adjacent nanoseconds at long timestamps.
- [ ] avoid an exhaustive multi-hour frame render and retain overflow cases.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core validation/arithmetic, SDK preflight/prepared-frame and Python authoring regressions as applicable. Include rational rates, local/project time and invalid inputs. Run relevant just check, just python-test, schema checks and just docs-check; media facts use generated fixtures.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

## Performance measurement and optimization

Spec draft: [spec-performance.md](spec-performance.md).

### P01: [P1] Capture reproducible CPU and hardware-WGPU baselines

**Type:** measurement. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Versioned suites, raw samples, environment identity and compatible comparisons already exist; no new baseline was captured in this audit.

**What it delivers:** Comparable baseline artifacts identify CPU, real hardware WGPU, software correctness and encoded versus frame-only costs separately.

**Acceptance criteria:**

- [ ] Use existing canonical and focused suites with revision, hardware, FFmpeg and cold/prepared scope.
- [ ] retain raw repeats and explain noise without promising speedups.
- [ ] report unavailable hardware as outstanding rather than substituting a software adapter.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P02: [P1] Measure process memory alongside engine counters

**Type:** implementation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Benchmark resource fields are engine estimates, not process RSS or complete GPU memory.

**What it delivers:** Benchmark records distinguish measured process memory from estimated engine/GPU resource counters.

**Acceptance criteria:**

- [ ] Measure renderer and owned FFmpeg child-process memory with units, platform, collection method and sampling interval.
- [ ] represent unavailable measurements explicitly rather than zero.
- [ ] keep comparison compatibility and focused benchmark-record tests correct.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P03: [P1] Expose actionable preparation and frame-work profiles

**Type:** implementation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** RenderResult already reports stage timing; WGPU upload/submit/readback and CPU hot-path timing are present.

**What it delivers:** Users can isolate the remaining unaccounted evaluation, plan-build, allocation and media costs without conflating overlapping stages.

**Acceptance criteria:**

- [ ] Audit existing timers first and instrument only demonstrably missing scopes.
- [ ] measure disabled and enabled profiling overhead separately with appropriate resolution.
- [ ] validate report serialization and distinguish wall time, overlapping work and preparation.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P04: [P1] Prototype WGPU static reuse in scenes containing groups

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** WGPU submit_frame explicitly skips all static-layer cache candidates when any evaluated layer contains a group.

**What it delivers:** A minimal paired prototype tests whether safe caching can benefit nested scenes.

**Acceptance criteria:**

- [ ] Compare mixed static sibling/group and nested-static cases against cache-disabled pixels.
- [ ] validate distinct composition identities, masks/mattes, invalidation and in-flight completion safety.
- [ ] report paired hardware timing and memory or explicitly leave the hardware result unverified.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P05: [P2] Prototype frame-plan reuse for stable topology

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** P03.

**Current evidence:** Compiled topology is retained, but GpuFramePlan builds fresh operation and layer vectors each frame.

**What it delivers:** A measured prototype reuses only invariant plan work while preserving all frame-dependent values.

**Acceptance criteria:**

- [ ] Profile stable topology versus changing active sets before choosing reuse boundaries.
- [ ] compare random/backward access and transition/mask/group changes against the existing path.
- [ ] retain the approach only if evaluation/plan cost improves without stale parameters or higher unacceptable memory.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P06: [P2] Reduce measured frame and buffer allocation hotspots

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** P03.

**Current evidence:** CPU surface reuse and persistent WGPU resources exist; plan vectors and other frame-local collections still allocate.

**What it delivers:** One measured allocation hotspot is reduced with owned returned frames and bounded scratch.

**Acceptance criteria:**

- [ ] Identify one dominant allocation/copy site from profiles.
- [ ] compare allocation counts, retained memory and stage timings before/after.
- [ ] test repeated frames, caller-held frames, cancellation and shrinking/growing scene work.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P07: [P2] Evaluate backend choice and worker scheduling tradeoffs

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** P01, P02.

**Current evidence:** Automatic selection, CPU workers and bounded WGPU submissions already exist.

**What it delivers:** Evidence-based recommendations explain which workload/worker configurations improve throughput within memory limits.

**Acceptance criteria:**

- [ ] Compare static/dynamic, decode-heavy and effect-heavy workloads on identified adapters.
- [ ] include CPU thread count, GPU depth, decoder duplication and process memory.
- [ ] preserve explicit backend choice and surfaced fallback errors in any proposed policy.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P08: [P2] Assess additional effect-pass fusion

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** P03.

**Current evidence:** Core already normalizes identity effects and fuses a compatible static colour chain.

**What it delivers:** One representative unfused chain has a correctness-checked cost assessment and optional minimal prototype.

**Acceptance criteria:**

- [ ] Record existing fusion boundaries before selecting a new candidate.
- [ ] compare order, clipping, alpha and intermediate rounding to the unfused reference.
- [ ] report pass count, timing and memory with no blanket fusion rewrite.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P09: [P3] Assess ROI and tiled execution for a bounded workload

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** CPU Gaussian work already uses nonzero-alpha bounds; generic ROI/tiled rendering is not established.

**What it delivers:** A bounded experiment identifies whether one sparse workload benefits from extending spatial work reduction.

**Acceptance criteria:**

- [ ] Choose one effect with finite support and an independent full-frame reference.
- [ ] test halos, borders, masks, transforms and transparent regions.
- [ ] report saved work, scan overhead, scratch memory and limitations before proposing general infrastructure.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P10: [P3] Assess SIMD for one measured CPU kernel

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** CPU byte-oriented kernels and parity contracts exist; SIMD speedup is unproven.

**What it delivers:** One portable hotspot has a measured SIMD feasibility result.

**Acceptance criteria:**

- [ ] Profile and select one kernel before modifying it.
- [ ] verify scalar equivalence at tails, unaligned inputs and alpha extremes.
- [ ] compare release builds and CPU feature availability without adding a dependency or requiring unsupported instructions by default.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### P11: [P2] Assess aggregate memory budgets and transient lifetimes

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** P02.

**Current evidence:** Caches, working textures, staging slots and worker resources are bounded separately; counters do not prove a whole-process bound.

**What it delivers:** A resource inventory and stress measurements expose peak overlap and recommend the smallest budget/lifetime improvement.

**Acceptance criteria:**

- [ ] Account for decoded assets, active cursors, worker scratch, in-flight readbacks, pending static textures and encoder payloads.
- [ ] test eviction/abort/reprepare and completion-token safety.
- [ ] distinguish enforcement targets from estimates and avoid an unmotivated generic allocator.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused benchmark record/comparison tests and paired release measurements for the named stage; record raw samples, source/environment identity and measured versus estimated memory. Render-changing prototypes also require frame correctness and actual hardware evidence for hardware claims.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

## Media pipeline and encoding

Spec draft: [spec-media.md](spec-media.md).

### M01: [P1] Define public encoder configurability

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Output quality selects CRF and x264 preset internally; encoding is libx264/yuv420p MP4 with no public independent preset control.

**What it delivers:** A reviewed minimal output-encoding contract builds on existing quality defaults.

**Acceptance criteria:**

- [ ] Determine required preset/quality/rate-control knobs and supported combinations from concrete use cases.
- [ ] preserve current output defaults and reject invalid combinations before execution.
- [ ] identify Rust/Python/JSON/CLI, inspection, schema and fixture changes without committing to arbitrary FFmpeg arguments.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M02: [P2] Assess hardware encoding capability and fallback

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** The current sink selects libx264; hardware codec selection is not exposed.

**What it delivers:** A device-specific feasibility report and minimal encode prototype compare hardware and software output.

**Acceptance criteria:**

- [ ] Report codec availability, driver/platform, supported pixel formats and actual selected encoder.
- [ ] compare quality, throughput, initialization and resource cost on identical input.
- [ ] specify explicit-request failure versus opt-in fallback and leave unavailable platforms unverified.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M03: [P2] Assess hardware video decode on a representative source

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Native FFmpeg timestamp-based decoding and RGBA conversion exist; hardware decode integration is unestablished.

**What it delivers:** A focused hardware-decode experiment measures decode/conversion costs without changing source-time semantics.

**Acceptance criteria:**

- [ ] Use VFR/B-frame and same-source-offset fixtures with software-selected-frame references.
- [ ] measure transfer/conversion overhead as well as decoding and peak memory.
- [ ] document device ownership, seek/flush and unsupported-codec behavior before proposing integration.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M04: [P3] Assess GPU-to-encoder transfer paths

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** M02.

**Current evidence:** FrameSink consumes CPU-accessible completed frames and WGPU currently reads back RGBA.

**What it delivers:** A feasibility result describes one realistic hardware-encoder interop path and its end-to-end costs.

**Acceptance criteria:**

- [ ] Use a demonstrated hardware encoder from M02.
- [ ] compare readback/upload baseline with device-compatible transfer and include synchronization/format conversion.
- [ ] retain portable CPU sink support and identify platform-specific constraints before any zero-copy claim.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M05: [P1] Profile decoder reuse and source scalability

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** CPU pools primary cursors per asset, extra concurrent cursors are retired, and WGPU has bounded native prefetch.

**What it delivers:** Evidence identifies remaining decoder churn or excessive retention across many placements and assets.

**Acceptance criteria:**

- [ ] Compare serial clips, simultaneous offsets, worker counts, sparse/backward access and long idle gaps.
- [ ] measure decoder creation/seeks, retained contexts, file handles, cache bytes and process memory.
- [ ] recommend only demonstrated improvements without duplicating existing reuse.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M06: [P2] Assess render/encode overlap under sustained load

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** P01, P02.

**Current evidence:** Dynamic WGPU already overlaps one ordered write with a subsequent submission; the coordinator and CPU workers are bounded.

**What it delivers:** A paired experiment determines whether tuning existing overlap improves a representative bottleneck.

**Acceptance criteria:**

- [ ] Compare existing scheduling with one bounded alternative and account for wall time versus overlapping timers.
- [ ] test ordered frames, slow/failed writes, cancellation and error precedence.
- [ ] quantify extra memory and retain the existing path if gains are not repeatable.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M07: [P1] Validate multi-hour muxing, large files and AV sync

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** T01.

**Current evidence:** Temporary output publication, frame ordering and audio mixing exist; multi-hour output was not exercised in this audit.

**What it delivers:** A gated media validation workload detects long-run timestamp, container and AV drift failures.

**Acceptance criteria:**

- [ ] Render a real timeline exceeding the former caps with generated low-cost video and identifiable audio markers.
- [ ] inspect frame/packet timestamps, duration and audio offset with ffprobe at early/middle/final points and cross a large-file boundary where feasible.
- [ ] verify finalization, overwrite safety and cleanup without committing output media.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M08: [P1] Define selected-range video rendering

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Prepared single-frame random access exists; video render requests expose output/overwrite/progress but no timeline range.

**What it delivers:** A reviewed range-render contract specifies output time, source time, audio and analysis semantics.

**Acceptance criteria:**

- [ ] Define half-open frame/time bounds, rational rounding, empty/out-of-range behavior and whether encoded timestamps restart at zero.
- [ ] compare selected output with the same frames/audio in a full render including transitions, nested effects and analysis history.
- [ ] identify a minimal shared SDK-to-CLI/Python slice before adding APIs.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M09: [P3] Design segmented and resumable rendering

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** M08.

**Current evidence:** Atomic publication and cancellation cleanup exist; no resume manifest or segment compatibility contract is established.

**What it delivers:** A design/prototype decision assesses safe resumability using approved range semantics.

**Acceptance criteria:**

- [ ] Define project/source/encoder identity, segment boundaries, audio continuity and reproducibility requirements.
- [ ] exercise interrupted or corrupt segments, disk failure and changed inputs without overwriting a good output.
- [ ] compare reassembly with a continuous render before committing to a manifest format.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### M10: [P3] Assess additional containers and codecs

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Semantic output validation currently requires .mp4 and the encoder selects H264/yuv420p.

**What it delivers:** A prioritized capability matrix recommends a first additional output combination or defers it.

**Acceptance criteria:**

- [ ] Evaluate concrete needs such as alpha-capable output, image sequences, WebM or intermediate/lossless video.
- [ ] record codec licensing/build availability, pixel formats, audio and dimension constraints.
- [ ] preserve MP4 defaults and separate desired support from verified platform/backend capability.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused media/SDK tests with generated inputs and ffprobe output checks; include failure cleanup. Changed public contracts require schema/typing/CLI checks. Hardware prototypes name actual codec/device; unavailable hardware remains unverified.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

## Architecture and maintainability

Spec draft: [spec-architecture.md](spec-architecture.md).

### A01: [P1] Guard shared capability declarations against drift

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Core effect descriptors and Python SourceCapabilities already centralize parts of support metadata.

**What it delivers:** Contract tests detect disagreement between canonical descriptors, public schema, authoring and backend support claims.

**Acceptance criteria:**

- [ ] Exercise representative effects and source scopes through existing exports/schema/validation.
- [ ] distinguish implemented, unavailable and unverified backend claims.
- [ ] add only missing cross-surface assertions and preserve intentional capability differences.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused descriptor/schema/export/authoring or prepared-lifecycle tests at existing seams. Before any accepted refactor lands, run relevant just check, just python-test, just docs-check and GPU checks if renderer ownership changes.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### A02: [P2] Simplify one effect integration path

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** The effect pipeline already shares descriptors and a backend-neutral pass representation; integration still spans several owners.

**What it delivers:** A concrete integration map identifies redundant declarations and proposes one small simplification.

**Acceptance criteria:**

- [ ] Trace one existing ordinary effect and one multipass effect through authoring, validation, compile/evaluate and both renderers.
- [ ] quantify duplicated decisions rather than counting files alone.
- [ ] preserve validation, schema, errors and backend boundaries and avoid a new plugin framework.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused descriptor/schema/export/authoring or prepared-lifecycle tests at existing seams. Before any accepted refactor lands, run relevant just check, just python-test, just docs-check and GPU checks if renderer ownership changes.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### A03: [P1] Consolidate resource-policy ownership

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** ResourceLimits is configurable internally, while public Editor validation constructs defaults and output dimensions also have semantic bounds.

**What it delivers:** A reviewed policy map distinguishes canonical constraints, caller budgets and device/execution limits.

**Acceptance criteria:**

- [ ] Identify every duration/dimension/cache/audio-source bound and its effective enforcement point.
- [ ] recommend a single owner per policy without moving I/O into core.
- [ ] specify backward compatibility and actionable error context, with public exposure handled by D02.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused descriptor/schema/export/authoring or prepared-lifecycle tests at existing seams. Before any accepted refactor lands, run relevant just check, just python-test, just docs-check and GPU checks if renderer ownership changes.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### A04: [P2] Identify a justified rendering or compiler module split

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Large coordinator/backend/compiler modules retain distinct responsibilities but a broad rewrite has no demonstrated benefit.

**What it delivers:** One bounded refactor proposal isolates a measured maintenance risk and names protected behavior.

**Acceptance criteria:**

- [ ] Choose one module using concrete coupling, duplicated logic or regression evidence.
- [ ] name callers, ownership and existing tests before proposing boundaries.
- [ ] produce independently green expand/migrate/contract slices only if required, otherwise recommend no refactor.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused descriptor/schema/export/authoring or prepared-lifecycle tests at existing seams. Before any accepted refactor lands, run relevant just check, just python-test, just docs-check and GPU checks if renderer ownership changes.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### A05: [P1] Strengthen prepared-resource ownership and teardown contracts

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** PreparedProject owns renderer/decoder state; frames own pixels, cancellation invalidates submitted work and GPU reuse follows completion tokens.

**What it delivers:** Public lifecycle regressions protect frame ownership, busy state, teardown and failure cleanup.

**Acceptance criteria:**

- [ ] Hold returned frames while rendering later/random frames and dropping original project/editor.
- [ ] test concurrent/busy misuse, repeated cancellation/abort and prepared-state reuse rules.
- [ ] verify native worker/process cleanup and no early GPU resource reuse at existing test seams.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused descriptor/schema/export/authoring or prepared-lifecycle tests at existing seams. Before any accepted refactor lands, run relevant just check, just python-test, just docs-check and GPU checks if renderer ownership changes.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### A06: [P2] Assess render-plan abstraction limits from concrete workloads

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** The pass IR has fixed logical temporaries and original-image retention rather than an arbitrary render graph.

**What it delivers:** A constraint analysis determines whether any planned workload needs a minimal IR extension.

**Acceptance criteria:**

- [ ] Use existing complex effects and proposed fusion/ROI/precision use cases as examples without treating them as committed features.
- [ ] check aliasing, pass order, lifetime and preparation limits in both backends.
- [ ] recommend the smallest extension only for an approved demonstrated need.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused descriptor/schema/export/authoring or prepared-lifecycle tests at existing seams. Before any accepted refactor lands, run relevant just check, just python-test, just docs-check and GPU checks if renderer ownership changes.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

## Correctness and quality

Spec draft: [spec-quality.md](spec-quality.md).

### Q01: [P1] Expand fractional-FPS and timestamped media regressions

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Rational timeline math, PTS selection, cache-boundary and VFR tests already exist.

**What it delivers:** A compact media matrix protects source selection and audio alignment at fractional output rates.

**Acceptance criteria:**

- [ ] Cover VFR/B-frames, nonzero source origins, sparse/backward reads and exact EOF under 24000/1001 and 30000/1001.
- [ ] compare selected source identities and decoded audio markers rather than only nominal FPS.
- [ ] retain reproducible generated fixtures and distinguish container rounding from engine errors.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### Q02: [P1] Stress bounded resources on long and repeated renders

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** T01.

**Current evidence:** Caches, worker pools, PCM chunking and staged capacity already exist; memory estimates alone cannot establish stability.

**What it delivers:** Gated stress workloads reveal duration-dependent retention and repeated-operation leaks.

**Acceptance criteria:**

- [ ] Sample memory/handles/decoder counts across a long dynamic render and repeated prepare/render/abort cycles.
- [ ] assert structural bounds where reliable and report noisy RSS separately.
- [ ] include audio-reactive analysis growth without assuming all retained signal data must be constant-sized.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### Q03: [P1] Expand temporal CPU and WGPU visual parity coverage

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Static/parity/stylization fixtures and separate hardware validation already exist; the support matrix still marks some effects/transitions not fully visually verified.

**What it delivers:** Sequence-based regressions cover currently under-verified temporal interactions.

**Acceptance criteria:**

- [ ] Select gaps from the current support matrix and test before/during/after transition and effect boundaries.
- [ ] compare CPU, strict software WGPU and separately identified hardware with justified tolerances.
- [ ] inspect exported frames/contact sheets and keep unsupported claims unchanged when hardware is missing.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### Q04: [P1] Expand corrupt-media and failure-recovery tests

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Existing tests cover sink ordering, abort, bounded stderr, failed writes/finalization and publication behavior.

**What it delivers:** Representative damaged media and mid-operation failures produce actionable errors and leave valid destinations intact.

**Acceptance criteria:**

- [ ] Exercise truncated/corrupt input, unavailable source after preparation, decode/encode error, disk/publication failure and cancellation.
- [ ] assert joins, temporary cleanup, error precedence and no false completed result.
- [ ] reuse existing injection seams and distinguish unsupported fixtures from regressions.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### Q05: [P1] Protect alpha semantics across composition stages

**Type:** verification. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** The current byte-oriented pipeline uses straight-alpha working textures with selected premultiplied internal operations.

**What it delivers:** A focused cross-backend fixture matrix catches hidden-RGB and edge contamination.

**Acceptance criteria:**

- [ ] Cover transparent colored texels, semitransparent transformed edges, nested opacity, masks/mattes and effect ordering.
- [ ] compare identity/no-op behavior and repeated/random access.
- [ ] require justified tolerances and actual frame inspection without silently changing the established alpha contract.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### Q06: [P2] Define explicit colour-management requirements

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Current working textures use encoded byte-space RGBA; this is explicitly not a linear-light or full colour-management promise.

**What it delivers:** A reviewed colour contract identifies user needs and a compatibility path.

**Acceptance criteria:**

- [ ] Compare metadata-only tagging, source conversion and linear-light processing with representative reference images.
- [ ] specify transfer functions, primaries, range, alpha interaction and output signaling.
- [ ] retain current default appearance unless a migration is explicitly approved.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### Q07: [P3] Assess higher-precision and HDR processing

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** Q06.

**Current evidence:** The working format is Rgba8Unorm and intermediate writes clamp/quantize; HDR/high precision is not an established contract.

**What it delivers:** A bounded prototype evaluates one higher-precision path against approved colour requirements.

**Acceptance criteria:**

- [ ] Measure banding/clipping and CPU/GPU numerical differences on reference gradients and highlights.
- [ ] include intermediate memory, adapter/storage-format limits and encoder signaling needs.
- [ ] propose opt-in scope only after Q06 and report SDR compatibility rather than claiming complete HDR support.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### Q08: [P2] Assess resampling and preview-versus-final quality

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Preview dimensions, quality tiers and effect-specific filters exist; they do not establish a universal quality strategy.

**What it delivers:** A visual/cost comparison recommends one narrowly scoped quality improvement.

**Acceptance criteria:**

- [ ] Compare subpixel transforms, down/up-scaling, group edges and effect neighborhoods against a reference.
- [ ] measure aliasing, alpha fringes, temporal stability and render cost.
- [ ] keep encoder preset quality separate from render resolution/filter quality and require approval for default changes.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Focused core/media/public render regressions as applicable, strict software-WGPU validation and separately identified hardware runs for GPU claims; inspect sequence frames and preserve justified tolerances. Keep expensive stress runs explicit and gated.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

## Python API, tooling and developer experience

Spec draft: [spec-developer-experience.md](spec-developer-experience.md).

### D01: [P1] Expose resolved visual timeline inspection

**Type:** implementation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Inspection reports resolved output and audio; high-level authoring duration remains intent and may differ from effective execution.

**What it delivers:** Users inspect local/project visual intervals and resolved output timing through the shared SDK, Python and CLI.

**Acceptance criteria:**

- [ ] Report resolved duration/frame count plus nested local placement and effective clipped spans without rendering all frames.
- [ ] include source-time mapping and diagnostic association where already resolved.
- [ ] keep existing report fields compatible and distinguish unknown media facts from resolved values.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Public SDK, CLI and Python report/typing/lowering tests as applicable; preserve structured diagnostics, unknown-value behavior and compatible serialized fields. Run relevant just check, just python-test and just docs-check.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### D02: [P1] Define public resource-limit configuration

**Type:** investigation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** A03.

**Current evidence:** ResourceLimits exists internally but ordinary Editor validation uses defaults and Python exposes no comparable limit options.

**What it delivers:** A reviewed configuration contract lets callers set finite budgets consistently across validation, preflight and preparation.

**Acceptance criteria:**

- [ ] Build on A03 ownership and distinguish execution budgets from canonical project data.
- [ ] define invalid/zero/unbounded setting semantics, discoverability and diagnostic reporting.
- [ ] map Rust/Python/CLI changes without silently changing serialized project meaning.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Public SDK, CLI and Python report/typing/lowering tests as applicable; preserve structured diagnostics, unknown-value behavior and compatible serialized fields. Run relevant just check, just python-test and just docs-check.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### D03: [P2] Assess a minimal timeline manipulation API

**Type:** prototype. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Mutable layer start/duration and sibling transition collections already exist.

**What it delivers:** A small prototype validates the most useful timeline-edit operation without introducing an editor framework.

**Acceptance criteria:**

- [ ] Select one concrete trim/move/split or sequence workflow and compare existing explicit mutations.
- [ ] preserve nested local time, source offsets, keyframes, transition references and atomic failure.
- [ ] propose exact behavior and compatibility before publishing a new helper family.
- [ ] Deliver a reproducible investigation/prototype result, alternatives, recommendation and explicit accept/defer/reject rationale. No production integration or new public behavior is authorized by completion of this ticket. Record unverified environments and request a reviewed implementation slice if the approach is accepted.

**Verification:** Public SDK, CLI and Python report/typing/lowering tests as applicable; preserve structured diagnostics, unknown-value behavior and compatible serialized fields. Run relevant just check, just python-test and just docs-check.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### D04: [P1] Expose backend capabilities and selection reasons

**Type:** implementation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** None (can start immediately).

**Current evidence:** Adapter metadata, explicit preferences, fallback diagnostics and effect descriptors exist; there is no unified high-level capability report.

**What it delivers:** Callers discover actual backend/adapter availability and supported operations before an expensive render.

**Acceptance criteria:**

- [ ] Reuse canonical capabilities and runtime adapter facts.
- [ ] distinguish supported, unsupported, unavailable and unverified rather than promising hardware parity.
- [ ] expose structured discovery and selection/fallback reasons through shared SDK/Python/CLI with safe no-adapter behavior.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Public SDK, CLI and Python report/typing/lowering tests as applicable; preserve structured diagnostics, unknown-value behavior and compatible serialized fields. Run relevant just check, just python-test and just docs-check.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.

### D05: [P2] Make performance reports useful for render diagnosis

**Type:** implementation. **Category/state:** enhancement / ready-for-agent.
**Blocked by:** P02, P03.

**Current evidence:** RenderResult and benchmark JSON already contain stage/resource/backend data; complete process-memory and finer profiling are separate planned additions.

**What it delivers:** Users can inspect a coherent performance report without parsing unrelated tracing output.

**Acceptance criteria:**

- [ ] Expose measured versus estimated memory, stage scope, wall time, overlapping work and actual backend consistently in SDK/Python/CLI.
- [ ] retain current report fields and explicit missing values.
- [ ] include one reproducible example and report-contract tests with P02/P03 data.
- [ ] Preserve existing compatible Rust/Python/JSON/CLI behavior outside this scope; update affected typing, exports, schema, fixtures and user examples with the change. A bug fix starts with the motivating regression. Do not weaken checks.

**Verification:** Public SDK, CLI and Python report/typing/lowering tests as applicable; preserve structured diagnostics, unknown-value behavior and compatible serialized fields. Run relevant just check, just python-test and just docs-check.

**Out of scope:** Other workstreams and adoption of unresolved approaches; examples/docs are included only when needed by this deliverable.
