# Demand-driven CPU scratch implementation plan

> For agentic workers: use superpowers:subagent-driven-development with a fresh implementer and scoped review. Follow this repository's plan lifecycle and the user's existing execution authorization.

Status: in progress
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
- [ ] Freeze exact candidate source/native identity after focused/broad correctness checks. Preserve the PR #79 baseline and its binaries; use a new output directory.
- [ ] Reuse existing benchmark smoke/canonical/MoviePy builders, process-tree sampler and output validator. No new benchmark framework. Compare raw samples and medians, simultaneous tree RSS/PSS, late-window memory, worker counts, decoder/cache metrics and exact input/encoder/runtime identities.
- [ ] Include basic/editing/layered 1080p, 60-second sequential, relevant 4K and five renders in one process. Use valid preserved controls where comparable; rerun controls if environment or small overlapping differences require it. Explain different instrumentation/build scopes explicitly.
- [ ] Fully validate published outputs and compare encoded/raw pixels against preserved baselines; inspect representative masked/effect/group frames. No hidden backend fallback or quality/preset reductions.
- [ ] Require measured actual memory improvement and no meaningful throughput regression, including comparison with the original pre-accounting baseline. If a trade-off remains, leave unmerged and report it; do not silently change the budget policy.

## Task 3: review and handoff
- [ ] Coordinator performs scoped spec/quality review, resolves concrete findings and obtains a final whole-branch review. Preserve issue scope; no speculative #76/#77 work.
- [ ] Transfer durable contracts to permanent docs and remove this completed active plan before merge. Keep raw reports outside tracked source.
- [ ] Publish focused changes to `develop` when prerequisites/integration state permit; never merge without explicit user approval. Avoid including parent changes as unrelated work in a final #75 diff.

## Progress and verification
October 25: required RED regression failed at the exact parent with retained
buffers 8 versus expected 1 (release CPU-only; saved under the ignored SDD task
workspace). Lazy first-use pool, resize reset, owned transfers, topology-based
scratch allowance and reachable retained union are implemented. Focused release
CPU suite now passes: 232 tests, 9 existing ignored. Regression coverage includes
late evaluated effect/mask/feather activation with fresh-frame equality, random
phase order, alpha reference pixels, held outputs and optional resize/reuse.
Full `just check` passed: 1,117 Rust tests / 26 existing ignored, all-feature
check and warning-denying Clippy, schema validation/freshness and documentation.
Canonical software Vulkan (llvmpipe LLVM 21.1.8, Mesa 26.1.5) passed its workspace
and strict 92-test renderer stages. Strict Vulkan hardware (NVIDIA GTX 1650 SUPER,
driver 595.71.05, discrete GPU) passed its workspace, strict 92-test renderer and
encoded CLI parity stages. Full `just python-test` passed 845 tests and compileall.
The final optimized native was force-reinstalled and loaded/hash-verified; recipe
sync preserved its hash. Candidate comparison packages were restored. Root's
requested normal merge of `origin/develop` (#73/#80, script/docs only) is retained
as `0158ef4`, preserving exact #79 ancestry. GL/WSL compatibility, extra ignored
stress and measurement acceptance remain unverified. Detailed commands, logs,
source/native/SDK identities and limits are in the ignored Task 1 SDD report.
Performance acceptance remains coordinator work and has not been claimed. Parent integrated evidence includes all-feature compilation, CPU namespace 165 passed/6 ignored, SDK report 2 passed, prepared-operation 1 passed, focused Python 21 passed and a byte-identical final-native 1080p smoke. These prove the parent only, not #75.

## Decisions and handoff
Use the existing issue as the authoritative design and repository-native active-plan path. User authorization permits #75 implementation now, while #79, #76 and #77 remain gated as stated. Coordinator schedules all heavy work and will preserve measured source/native identities and unresolved acceptance decisions.
