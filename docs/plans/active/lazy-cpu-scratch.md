# Demand-driven CPU scratch implementation plan

> For agentic workers: use superpowers:subagent-driven-development with a fresh implementer and scoped review. Follow this repository's plan lifecycle and the user's existing execution authorization.

Status: worker-policy research complete; no safe focused replacement justified, performance acceptance and parent integration remain gated
Branch / baseline: `perf/lazy-cpu-scratch` / `06325fae2e4291da320da879a75d49c6a3b2bc95` (draft PR #79).

## Objective
Implement [#75](https://github.com/evgen2571/vestra/issues/75), under [#71](https://github.com/evgen2571/vestra/issues/71): optional effect/mask buffers allocate only when used, reuse safely, and improve actual memory without meaningful throughput regression.

## Baseline and authorization
PR #79 contains corrected capacity accounting and worker estimates. Its fixed eight-buffer scratch allowance reduces workers and measured throughput. The measured revision `7d640b5` and integrated head `06325fa` have preserved native artifacts and reports outside Git.

On October 25 the user explicitly authorized: "Allow #75 against unmerged #79; keep #76–77 blocked (Recommended)". This is an exception only to #75's prerequisite gate. PR #79 stays unmerged; no merge approval exists. Do not change GitHub's spec or native dependency links. The original request already authorizes implementation decisions, native subagents and dedicated worktrees; it forbids new specifications. This plan implements the existing issue rather than creating another spec or approval stage.

## Architecture and file ownership
Keep the CPU renderer's existing surface pool, logical effect bindings and ownership. Retain the one always-required current canvas and allocate the other optional images/vectors on first actual use. Retain allocated storage for safe reuse at matching dimensions; resize must preserve logical extent and clear stale bindings/content. The existing reachable-plan storage walk supplies a conservative allowance for optional scratch that can become retained during rendering.

- `crates/vestra-render/src/cpu/surfaces.rs`: lazy images/vectors, binding invariants, resize/take/reuse, actual capacities and allocation events; adjacent unit tests.
- `crates/vestra-render/src/cpu/memory.rs`: `estimated_worker_bytes(&RenderPlan) -> u64` reflects the changed scratch lifecycle, using existing compiled effect topology and reachable-demand traversal.
- `crates/vestra-render/src/cpu/{compositor,effects,worker}.rs`: preserve callers; change only what lazy ownership requires.
- `crates/vestra-render/src/cpu/{backend_tests.rs,effects/tests.rs}`, `crates/vestra/src/application/render.rs`, `python-tests/test_render_result.py`: update precise eager-specific expectations and retain pixel/ownership checks.
- `docs/development/architecture/cpu-renderer.md`: document actual allocation/reuse and estimator limits. Public model/schema and existing metric names/types remain unchanged.

## Global constraints
- "Unused optional buffers have no full-canvas capacity; buffers allocate only when needed and reuse safely without changing pixels."
- "Preserve failure cleanup, resource ownership/reuse and public contracts; no output buffer borrowing."
- "Demonstrate actual intended memory improvement without a repeatable meaningful throughput regression."
- "No unrelated refactors, new dependencies except a justified WGPU correction, output-quality changes, arbitrary concurrency limits or committed generated media/profiler dumps."
- Preserve local timing, effect order, alpha, masks/mattes, caller-held frames and source bounds. Do not change the 512 MiB heuristic, CPU reserve, worker cap, decoder cache policy or encoder settings to obtain a favorable result.
- One heavy build/test/benchmark/GPU window at a time, granted by coordinator. Preserve main checkout, old worktrees, binaries and all existing evidence. Use raw Git worktrees/native agents, not Orca orchestration.

## Review focus
1. A frame first activates an animated effect/mask/feather after earlier plain frames: allocate then, with identical pixels.
2. A prepared pool changes size after using optional storage: no stale extents/bindings or excess retained capacity attributed to unused buffers.
3. Original/current/temporary logical aliases in multi-pass effects remain safe when a physical slot has never allocated.
4. `take_current` and independently returned frames keep owned pixels across reuse, random access and renderer teardown.
5. Retained optional scratch can accumulate over separate timeline phases: the plan estimate must cover that retained union, reachable hidden mattes and nested/group masks, not just frame-zero features.

## Task 1: lazy pool, matching estimate and contract regressions
Interfaces remain the existing private `EffectSurfacePool` methods and public metric fields. Add the minimum internal state/helpers needed to remember extent and ensure storage at real use sites; avoid a second render graph or effect semantics table.

- [x] Read actual pool callers, compiled effect-pass requirements/topology, compositor mask/matte paths and relevant existing fixtures before editing.
- [x] Write and run a baseline-failing surface test: `EffectSurfacePool::new(4, 3)` retains only one required canvas buffer and 48 bytes; `current()` adds no allocation. A current-to-current copy pass allocates only its needed second image (96 bytes total), without mask/feather storage. Save RED output before production edits.
- [x] Add focused assertions for no-effect/no-mask, effect-only and mask-only allocations; zero-radius feather must not allocate blur vectors. After real use, repeated operations at the same extent stabilize allocations and preserve reference pixels.
- [x] Implement first-use allocation in the existing pool. Allocate destinations before pass execution, initialize mask coverage when first/reset requested, defer both feather vectors until positive feathering, and keep output transfer ownership independent. Stats count actual nonempty backing capacity and real allocation events.
- [x] Cover resize smaller/larger after optional use, animated activation and feathering, nested groups/mattes, alpha, random/repeated access and held outputs. Reuse existing fixture helpers/reference pixels; do not weaken tolerances or pixel assertions.
- [x] Adjust the existing estimator and its tests to the new lifecycle. Use existing compiled topology and reachable traversal; preserve saturating arithmetic, serial-video/hidden-layer invariance and separately budgeted caches. Estimate the union of retained optional storage when appropriate. Do not use stale fixed eight-buffer accounting or tune policy constants.
- [x] Update eager-only Rust/SDK/Python expectations to precise fixture counts/estimates and update the authoritative CPU renderer guide.
- [x] Run focused renderer regressions, then relevant `just check`, `just python-test`, formatting/docs and required parity checks in a granted window. Native builds must force reinstall (`uv sync --reinstall-package vestra` with `--profile release`) and verify the loaded artifact hash; uv can otherwise reuse a stale editable wheel.
- [x] Self-review, commit only this task's files using `perf: ...`, and report actual commands/results/limits. Do not push/create a PR or merge; coordinator owns independent reviews and publication.

## Task 2: coordinator measurements and acceptance
- [x] Freeze exact candidate source/native identity after focused/broad correctness checks. Preserve the PR #79 baseline and its binaries; use a new output directory.
- [x] Reuse existing benchmark smoke/canonical/MoviePy builders, process-tree sampler and output validator. No new benchmark framework. Compare raw samples and medians, simultaneous tree RSS/PSS, late-window memory, worker counts, decoder/cache metrics and exact input/encoder/runtime identities.
- [x] Include basic/editing/layered 1080p, 60-second sequential, relevant 4K and five renders in one process. Use valid preserved controls where comparable; rerun controls if environment or small overlapping differences require it. Explain different instrumentation/build scopes explicitly.
- [x] Fully validate published outputs and compare encoded/raw pixels against preserved baselines; inspect representative masked/effect/group frames. No hidden backend fallback or quality/preset reductions.
- [ ] Require measured actual memory improvement and no meaningful throughput regression, including comparison with the original pre-accounting baseline. If a trade-off remains, leave unmerged and report it; do not silently change the budget policy.

## Task 3: review and handoff
- [x] Coordinator performs scoped spec/quality review, resolves concrete findings and obtains a final whole-branch review. Preserve issue scope; no speculative #76/#77 work.
- [ ] Transfer durable contracts to permanent docs and remove this completed active plan before merge. Keep raw reports outside tracked source.
- [ ] Publish focused changes to `develop` when prerequisites/integration state permit; never merge without explicit user approval. Avoid including parent changes as unrelated work in a final #75 diff.

## Progress and verification
Implementation `9b9f9bb` passed the scratch baseline RED (8 versus 1), focused
CPU release tests (232 passed / 9 existing ignored), full `just check` (1,117
Rust passed / 26 existing ignored, all-feature compilation, Clippy, formatting,
schema and docs), 845 Python tests/compileall, and canonical strict software and
NVIDIA hardware Vulkan recipes (92 strict renderer targets each, plus hardware
encoded CLI parity). The normal develop merge `0158ef4` preserves exact #79
ancestry and approved #80 integration.

Whole-branch source review found one minor reuse-counter mismatch. Correction
`3d689f2` captured its own RED, restored first-allocation zero/second-pass one
reuse, and passed focused CPU 232/9, SDK report/JSON 2 and prepared-operation 1,
formatting, renderer all-feature/all-target warning-denying Clippy and docs.
Scoped re-review is clean. Historical broad results remain evidence for `9b`;
they were not needlessly rerun for the counter-only correction.

Final source fingerprint is
`58b41f81a4b1d838c4e7f72e26870ce82e1c7dc0f683086fc715fbbe4e0a28a2`;
loaded optimized native is
`e4d2a89b57ecb9951aefcacce27f9c74ee70d89e3570892a05eb2f9d66007bf6`.
Forced reinstall, final SDK identity and matching comparison package versions
were verified. Actual final-native Python frames/video captures and final
optimized SDK companions pass; source/native stayed frozen during sampling.
No open source-review finding remains.

## Decisions and handoff
Use the existing issue as the authoritative design and repository-native active-plan path. User authorization permits #75 implementation now, while #79, #76 and #77 remain gated as stated. Coordinator schedules all heavy work and will preserve measured source/native identities and unresolved acceptance decisions.


## Review follow-up: scratch reuse metrics
October 25: final source review found that a first-use effect destination was
counted as both allocation and reuse, contrary to the existing public metric
contract. A baseline RED test observed reuse 1 versus expected 0 after the first
allocating copy. The minimal correction counts reuse only for an existing
destination; the second copy reports one reuse. Updated the affected pass-chain,
Glow and 100-frame Gaussian expectations while preserving all pixel assertions.
Focused CPU release tests passed 232 / 9 ignored; SDK report/JSON tests passed 2,
prepared-operation metrics passed 1, formatting and renderer all-feature/all-target
warning-denying Clippy passed. Forced optimized native reinstall and loaded
artifact verification completed; candidate comparison packages were restored.
No broad workspace/GPU/Python reruns were needed for this counter-only correction.
Exact commands, final identities and commit are in the ignored Task 1 fix report.

Coordinator reports original candidate `9b9f9bb` did not meet #75 acceptance:
layered wall time was about 6.8% higher and 4K PSS about 2.8% higher. These are
coordinator results, not new worker measurements. Final-candidate captures are now complete; the measured acceptance result below
supersedes this initial checkpoint. #79 stays unmerged and #76/#77 blocked.
No worker-budget, reserve, cap, cache, decoder or encoder policy was tuned.

## Final measurement outcome and handoff

Correctness and measurement execution are complete. Performance acceptance is
**failed**: a fresh paired confirmation shows layered rendering takes 12.6%
longer with six instead of eight workers. The unchanged 512 MiB heuristic, CPU
reserve, worker cap, cache policy and encoding settings were not tuned.

| 1080p workload | Median process seconds | Median peak PSS MiB | Workers |
| --- | --- | --- | --- |
| basic | 9.946 → 9.890 | 1240.7 → 1170.8 | 8 → 8 |
| editing | 13.350 → 13.292 | 1568.9 → 1514.3 | 8 → 8 |
| layered | 19.409 → 21.848 | 1772.8 → 1562.6 | 8 → 6 |

Basic/editing timings overlap baseline variation while PSS falls 5.6%/3.5%.
Every final measured layered time exceeds every paired baseline time; PSS falls
11.9% at the throughput cost above. Initial `9b` captures also showed a layered
slowdown. System timing shifted between batches, so short pairs were repeated
rather than attributing a gain to the counter-only correction.

The 60-second, 4K and five-render-process cases were also captured on final
`3d689f2`, reusing compatible fresh original controls. Their timing phase differs,
so raw wall deltas are observations rather than isolated causal speedup claims.
Four-K uses four instead of two workers and is much faster in both candidate
builds, but its final median PSS is about 0.8% higher with overlapping sample
ranges; no reliable 4K resident-memory improvement is established. All idle
thread observations in repeated-process diagnostics return to one; allocator
variation is not a leak/retention diagnosis or a #77 completion claim.

The selected final comparison contains 42 process records, 50 MP4s and 27,480
fully decoded frames. Every file has the exact baseline encoded hash for its
workload. Final SDK companion outputs also fully decode and match those bytes.
Final direct-frame validation covers random/repeated requests and held ownership:
32 RGBA comparisons, CPU exact and NVIDIA Vulkan max channel error 1. Mask/matte
contact sheets and final basic/dissolve/layered encoded frames were inspected.

Full report and raw identities/samples are outside tracked source under
`~/orca/workspaces/vestra/reliability-evidence-20261024/cpu75-handoff.md` and
`cpu75-final-measurement-summary.json`; initial and final data remain separate.

Keep this active plan: acceptance and parent integration are unresolved. #79
stays draft/unmerged; no #75 PR was published because acceptance failed and its
parent remains unmerged. #76/#77 remain blocked under the explicit user choice.
The remaining material decision is the worker-policy/performance trade-off, not
a source correctness defect. No speculative policy or decoder/encoder change
was made. Remove the completed plan only before an actually accepted merge.

Unverified: compatible GL/WSL, Metal, DX12/browser runtime, extra ignored stress,
multi-hour/large-file soaks. Vulkan evidence does not establish those platforms.

## October 25: controlled layered investigation

User authorized four matched SDK arms (original eight, #79 forced eight,
previous #75 forced eight and automatic six), followed by a lifetime-backed
correction and repeated five-workload validation. Temporary overrides live
only in ignored source snapshots; production defaults remain unchanged.
Ranked hypotheses: concurrency loss; first-use lazy overhead; overestimated
overlapping storage. Preserve all previous binaries/reports and record new
raw timings, worker counts, peak/late RSS/PSS and full output validation.
No merge, new branch/PR or #76/#77 work is authorized.

### Controlled investigation outcome

The fixed-eight release SDK medians were original 19.507s, #79-only 19.385s
and previous #75 19.396s; automatic #75 six workers took 22.031s. Outputs
were identical. The repeatable loss is concurrency, not lazy-allocation overhead.

The estimate summed incoming RGBA conversions that cannot coexist within one
synchronous CPU worker. Retain current and scaler storage per cursor, but add
only the largest incoming conversion. Three 720p sources with four 1080p scratch
owners and two SDK output allowances change 82,944,000 to 75,571,200 bytes per
worker, permitting seven. Eight still charge 576.56 MiB against the unchanged
512 MiB heuristic. Mask storage and SDK output ownership are real. Cached current
aliases do not justify dropping allowances for zero-cache/extra-cursor/provider
paths; no cache-aware contract or policy increase was invented to restore eight.

A focused RED observed 82,944,000 rather than 75,571,200. Correction passed 170
focused CPU tests (6 existing ignored), full just check (1,118 Rust / 26 ignored,
Clippy/build/format/schema/docs), 845 Python plus compileall, canonical smoke,
strict software Vulkan and NVIDIA Vulkan recipes (92 renderer targets each,
plus hardware encoded CLI parity). 32 fresh raw-frame comparisons passed:
CPU exact to original, hardware maximum channel error 1; held ownership passed.
Independent source and evidence reviews have no remaining findings.

Fresh paired medians (original / previous #75 / corrected):

| Workload | Seconds | Peak PSS MiB | Workers |
| --- | --- | --- | --- |
| basic | 9.880 / 9.863 / 9.828 | 1223.6 / 1148.9 / 1146.5 | 8 / 8 / 8 |
| editing | 13.317 / 13.336 / 13.303 | 1557.6 / 1493.8 / 1485.2 | 8 / 8 / 8 |
| layered | 19.373 / 21.812 / 20.360 | 1737.6 / 1516.3 / 1593.4 | 8 / 6 / 7 |
| sequential60s | 57.616 / 57.031 / 57.451 | 1287.5 / 1211.8 / 1203.9 | 8 / 8 / 8 |
| 4K | 95.773 / 54.978 / 53.638 | 2641.1 / 2637.8 / 2626.8 | 2 / 4 / 4 |

Layered remains 5.09% slower (every corrected sample exceeds every paired
original); PSS falls 8.29%. Basic/editing/sequential retain throughput and save
4.7–6.5% PSS. 4K is faster but overlapping PSS ranges establish no reliable
resident-memory saving. Two fresh five-render Python processes also pass all
outputs, with peak PSS 1720.7 to 1264.3 MiB and every idle thread count returning
to one; allocator variation is not a leak diagnosis or #77 completion.

The accepted matrix contains 60 MP4s / 36,720 decoded frames; fixed-worker and
repeated-process diagnostics bring accepted validation to 86 MP4s / 44,520 frames.
All files match their original encoded reference hashes. An initial 4K batch
coincided with unrelated Nix contention and was excluded wholesale, preserved,
and recaptured after the host settled. Raw timings, peak/late RSS/PSS, hashes,
commands, exclusions and full report remain outside Git in the worktree sibling
`reliability-evidence-20261024/cpu75-regression/`. Production sources contain no
diagnostic override; ignored source snapshots retain the temporary probe.

Keep #79 and #75 unmerged under the unchanged throughput requirement;#79 alone
previously showed 49.1% layered slowdown under automatic selection. Keep #76/#77
blocked. No new branch/PR, push, merge or main/develop change was made. The
validated correction improves accounting and partially recovers throughput, but
it does not satisfy performance acceptance. Keep this active plan.

## Memory-aware worker policy investigation

The user authorized reconsidering the fixed 512 MiB policy after 4b04a1b, with
real host/cgroup/process limits, concurrent renders and full-pipeline memory
requirements. This supersedes the earlier restriction against any justified
policy redesign; it does not permit arbitrary budget increases, weaker
accounting or benchmark-specific production settings. Preserve the current
policy until a replacement is justified. No new branch/PR, merge, main/develop
change or #76/#77 work is authorized.

Milestones: audit admission/ownership boundaries; measure original/latest and
a temporary CPU-throughput ceiling across five existing workloads; exercise
real constrained cgroups, process limits and simultaneous render admission;
choose a focused safe implementation or document why the boundary must be
restructured. Record workers, timing, peak/late RSS/PSS and effective headroom.
Use existing sampler/workloads/validators and preserve prior evidence.

### Policy decision and evidence

No production policy change. The per-worker estimate remains 75,571,200 bytes
for layered; seven fit the 512 MiB pixel heuristic, eight do not. A host-memory
getter or a percentage cannot turn this partial model into whole-render
admission. Decoded assets/audio analysis precede selection, encoder allocation
follows it, and independently prepared projects do not reserve future memory.
The fixed heuristic both leaves throughput unused and fails under real limits.

New medians from fresh release SDK processes in 4 GiB cgroups, swap disabled,
one warmup plus three measured trials per arm. Each cell is seconds / peak
process-tree PSS MiB; ceiling uses an ignored diagnostic eight-worker override.

| Workload | Original | Latest 4b04a1b | Diagnostic ceiling | Workers |
| --- | --- | --- | --- | --- |
| basic | 11.173 / 1245.4 | 10.981 / 1149.4 | 10.945 / 1170.1 | 8 / 8 / 8 |
| editing | 15.070 / 1574.6 | 14.734 / 1493.2 | 14.997 / 1500.2 | 8 / 8 / 8 |
| layered | 21.769 / 1769.1 | 22.431 / 1617.8 | 21.006 / 1773.1 | 8 / 7 / 8 |
| sequential60s | 64.369 / 1287.4 | 64.788 / 1232.2 | 63.295 / 1237.5 | 8 / 8 / 8 |
| 4K (contention affected) | 105.092 / 2565.2 | 67.159 / 2693.9 | 39.704 / 3081.3 | 2 / 4 / 8 |

Layered latest remains 3.04% slower, with non-overlapping timing ranges. Eight
recovers speed at essentially original PSS; it is viable for this fixture in
the measured 4 GiB scope, not proven safe for arbitrary projects. Basic/editing
save 7.71%/5.17% peak PSS; sequential saves 4.28%, with overlapping timing ranges.
An unrelated Nix system build overlapped 4K, with observed CPU/memory pressure.
Preserve every row, but do not use its precise 4K speed deltas for acceptance;
the earlier controlled same-source 4K comparison above remains separate.

All 60 matrix outputs / 36,720 frames fully decode and match per-workload
encoded reference hashes. Summary worker counts use matching SDK pipeline-depth
and allocated-slot fields: one non-atomic thread-name scan reported nine for
an eight-worker 4K trial. The raw scan discrepancy is retained.

Real isolated limit diagnostics established:

- Under 1 GiB, layered seven is OOM-killed while one completes in 95.69s
  at 982.1 MiB peak PSS. 4K is OOM-killed even at one worker.
- With 1 GiB RLIMIT_AS and 4 GiB physical allowance, layered seven allocator-
  aborts; one reaches an x264 allocation error returned by the SDK, then the
  diagnostic driver's unwrap panics. A physical-memory probe misses this limit.
- Two barrier-synchronized in-process prepared renders select 14 workers while
  initially using only 37–43 MiB RSS; they are OOM-killed at 2 GiB but both finish
  at 4 GiB (36.87s, 3274.2 MiB peak PSS, at least 818.6 MiB hard headroom).
- With memory.high 1408 MiB and memory.max 4096 MiB, seven workers hit the 240s
  test timeout with 13,998 reclaim events and no OOM, despite at least 2593.8 MiB
  hard headroom. Four complete in 29.35s without high events, with at least
  112.2 MiB pressure headroom. More workers can reduce throughput under pressure.
- Two-CPU affinity plus 1 GiB permits basic at one worker (48.04s, 704.7 MiB PSS),
  but 4K at one is OOM-killed. This is not validation on different hardware.

Recommend separating #79 resource accounting from worker-selection behavior.
Whole-operation requirements and owned outstanding reservations belong at the
SDK preparation/video seams, with a structured cannot-fit error. Keep host,
cgroup hard/pressure, and virtual limits distinct; avoid double counting already
resident storage. A process-local reservation cannot coordinate unrelated
processes. Do not fit encoder reserves or budget percentages to these fixtures.
No adaptive scheduler, configuration option, dependency or estimate adjustment
was added. The permanent CPU architecture guide records these constraints.

Full commands, identities, raw timings, peak/late RSS/PSS, launch/minimum/late
headroom, cgroup events/journals, considered approaches and limitations are in
the sibling `reliability-evidence-20261024/cpu75-policy/report.md` and its raw
records. Previous evidence is unchanged. Keep #79/#75 unmerged and #76/#77
blocked; this investigation does not satisfy performance/admission acceptance.

Fresh verification: just check with explicit Vulkan passes 1,118 Rust tests
(26 existing ignored), formatting, all-feature build, Clippy, schema and docs;
full Python passes 845 plus compileall. Raw frames pass 32 exact original/CPU
comparisons at normal/two-CPU affinity, plus 16 NVIDIA Vulkan comparisons with
maximum channel error one. Held ownership and random/backward/repeated requests
pass; the contact sheet was inspected. Five successful constrained outputs add
1,500 fully decoded frames. The two-CPU encoded basic has a different encoder
hash, with exact raw frame parity; other successful constrained outputs match.

Retained failed attempts: default graphics caused five EGL BadAccess SDK test
failures; explicit Vulkan passed. The first Python run omitted the cached Cargo
target and exhausted disk during its schema build (844 passed / one failed).
Only artifacts newly generated by that attempt were removed after a timestamp
check; the cached rerun passed. No production source or installed native change.

The earlier same-source strict software/hardware recipes and canonical smoke
were not repeated. Different physical hardware, cgroup v1, RLIMIT_DATA enforcement,
non-Linux memory/commit probes, GL/WSL, Metal/DX12/browser and multi-hour stress
remain unverified. Documented OOM/pressure failures are current limitations,
not passing safety tests. Review confirms only architecture and this plan change;
no new branch/PR, push, merge or main/develop change, and #76/#77 remain blocked.
