# Renderer crate extraction

Status: in progress
Branch / baseline: `refactor/render-crates`, stacked on completed Phase 2
`69e8eb6` (`refactor/render-boundaries`).

## Objective

Extract common/CPU/WGPU/facade renderer crates with unchanged rendering, public
SDK/Python/JSON and diagnostics, and decouple media completely from renderers.
The user's approved attachment and [review section L](render-extraction-review.md#l-approved-phase-3-amendments-2026-10-27)
supersede the original proposal. No issue or additional approval round is needed.

## Baseline and references

Initial working tree clean; sole existing checkout retained. Phase 2 is not in
local develop. main is `b23c985a623efea14511e25aeb1df93b495beecb`, develop is
`c26f52f4c41a7d010775c8da61df59bf7caa8cb5`; keep both unchanged.
Read AGENTS.md, README.md, CONTRIBUTING.md, PLANS.md, original review and execution
plan. Phase 2 commits preserve shape pixels, kernel shader mapping and GPU-only
resource tests. Existing measurement/frame/asset evidence remains under
`target/render-boundaries/` and `target/benchmark-results/`; do not overwrite it.
New evidence: `target/render-crates/`. Use pinned Nix, CARGO_BUILD_JOBS=2, and
separate software/NVIDIA adapter runtimes as recorded in review section K.

## Scope and non-goals

Move implementation ownership, narrow cross-crate interfaces, migrate tests,
update Cargo/CI/scripts/packaging/docs and adapt media/SDK sinks. No algorithms,
precision, shader bytes, worker heuristics, decoder/cache policy, FFmpeg behavior,
static-copy optimization, new registry/framework or fifth renderer crate.
No merge, push or PR. Preserve existing worktrees and unrelated changes.

## Constraints and decisions

- Common owns backend-independent lifecycle, prepared Arc assets, geometry,
  necessary shared pixel/numerical helpers and kernel requirements.
- CPU owns execution, workers, mutable caches, profiling and CPU capability
  validation; WGPU owns GPU execution/resources/shaders and GPU validation.
- Facade owns selection/fallback, compatibility reexports and parity tests.
  Defaults remain CPU+WGPU; no-backend and single-backend builds remain valid.
- Media depends on core and existing media libraries, never any renderer crate.
  Sink writes borrow frame bytes; SDK retains frame ownership and scoped overlap.
- Gaussian float weights are unconditional; calculations and max-16 cache stay
  unchanged. Review every public bridge; private fields and borrowed accessors
  take precedence over blanket visibility changes.
- Existing assertions/tolerances/goldens remain. Mechanical relocations reuse
  regression coverage; the changed borrowed sink API gets focused contract tests.
- Independent work may be delegated with explicit, non-overlapping file ownership;
  coordinator owns integration, commits, plan and serialized heavy validation.

## Milestones

- [ ] 1. Separate CPU timings, blend execution and host CRT; focused renderer tests.
- [ ] 2. Extract common with narrow accessors, unconditional Gaussian storage and
  shared mapping/requirements; common and facade feature checks. Review boundary.
- [ ] 3. Extract CPU with backend capability policy; CPU tests and isolation.
- [ ] 4. Extract WGPU/shaders, split private/parity tests without losing assertions;
  WGPU/facade tests, asset hashes and strict software/hardware execution.
- [ ] 5. Complete facade, media/SDK borrowed sink migration (distinct commit),
  test discovery and CI with meaningful backend isolation.
- [ ] 6. Complete compatibility, packaging, benchmark/memory/build measurements
  and permanent documentation; record final evidence and limitations.

## Acceptance criteria

Run all commands/criteria in review J except superseded cpu-kernels commands.
Additionally prove media normal/build/dev graphs have no renderer and isolated
media compilation excludes renderer preparation; identify independent font/image
dependencies accurately. Preserve static copies/metrics, asynchronous write
overlap, lifecycle/cancellation/cleanup/publication and exact diagnostics.
Account for relocated test names/assertions, not only aggregate totals.
Verify a fresh Python extension, full workspace checks, generator/JSON, SDK/CLI,
golden raw frames, strict software and actual NVIDIA, fresh wheel and sdist built
from extraction, all embedded assets/licenses, canonical CPU/WGPU performance,
memory/cache counters and cold/warm/incremental compilation. No tolerance changes.
Final report covers branch/commits, layout/graphs/ownership, both isolation axes,
facade/Python/JSON/SDK/CLI/coverage/parity/media results, benchmarks/memory/builds,
packaging/CI tradeoffs, remaining concerns/Phase 4 suggestions, unchanged protected
branches and clean tree. Distinguish actual results from unverified claims.

## Progress and verification

2026-10-27: Verified clean checkpoint/history/refs and created requested stacked
branch. Read the approved design and recorded amendments without altering Phase 2
measurements. Fresh no-backend baseline test started; result pending.

## Decisions and discoveries

2026-10-27: Keep the existing checkout/cache on the explicitly requested feature
branch. No additional worktree is needed. Media extraction may proceed independently
of renderer file moves; keep its diff and commit distinct.

## Completion / handoff

Implementation in progress. Required final validations are not yet performed.
Retain active evidence through handoff; follow PLANS.md transfer/removal before
any later, separately authorized merge.
