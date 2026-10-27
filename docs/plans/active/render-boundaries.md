# Renderer ownership cleanup and extraction review

Status: in progress — Phase 2 only; Phase 3 requires explicit user approval.
Branch / baseline: `refactor/render-boundaries`, fetched `origin/develop`
`c26f52f4c41a7d010775c8da61df59bf7caa8cb5`.

## Objective

Preserve rendering and public interfaces while removing demonstrated ownership
obstacles to a common/CPU/WGPU/facade crate split. Deliver the Phase 2 audit,
measured baseline and concrete Phase 3 design, then stop for design approval.
The user supplied the complete specification as `pasted-text-1.txt` in this
session; no GitHub issue was supplied. Do not create or update issues or PRs.

## Baseline and references

- Clean checkout initially on `develop`; the sole existing worktree is this one.
- Fetched upstream on 2026-10-27; local `develop` equals the baseline above.
- Local `main`: `b23c985a623efea14511e25aeb1df93b495beecb`.
- Phase 1 is present in upstream history: effect catalog/exporter, structured
  Python metadata, generated declarations/stubs/schema/reference, frozen fixtures,
  and generation check integration. Current checks must still verify them.
- Read [architecture](../../development/architecture/overview.md),
  [crate ownership](../../development/architecture/crates.md),
  [testing](../../development/testing.md),
  [performance](../../development/performance.md) and
  [GPU validation](../../development/gpu-validation.md).
- CodeGraph crashes with exit 139 even on `--help`; use source inspection.
- Native host reports Rust/Cargo 1.95.0 and GTX 1650 SUPER, NVIDIA 595.71.05.
  Hardware validation must prove the adapter selected by Vestra itself.

## Scope and non-goals

Audit shared contracts/resources, CPU/WGPU implementations, public consumers,
features, packaging, tests, CI and benchmarks. Fix only concrete extraction
blockers inside the existing renderer crate. No extraction, new render features,
algorithm/shader/scheduler/cache-policy changes, effect generation redesign,
registry generation, merge, push or PR.

## Constraints and decisions

- Preserve source and rendered output, import paths, diagnostics, resource
  sharing, cancellation and staged completion semantics.
- Keep the existing strongly typed core contracts and `RenderBackend` interface.
- Avoid blanket public fields, resource copies and generic framework abstractions.
- Read-only audit agents own independent shared/CPU, WGPU, and Cargo/consumer
  investigations; only the coordinator edits source and runs builds/benchmarks.
- Work in the clean existing checkout on the requested task branch. Keep local
  `main` and `develop` references unchanged.
- Detailed evidence belongs under ignored `target/render-boundaries/` and
  `target/benchmark-results/`; the review report belongs beside this active plan.

## Milestones

- [ ] Audit current ownership, callers, APIs, features, tests and Phase 1 integration.
- [ ] Establish current baseline: full Rust/Python/generation/schema, four
  renderer configurations, software/hardware WGPU, representative frames,
  canonical CPU benchmark and available memory/dependency/build measurements.
- [ ] Make minimal blocker cleanup in focused tested commits; no extraction.
- [ ] Repeat required validation and inspect actual rendered output; investigate
  failures without weakening tests, tolerances or golden references.
- [ ] Deliver ownership matrix, dependency diagrams, exact APIs/layout/features,
  test/CI migration, risks, commit-sized extraction plan and measured evidence.
- [ ] Stop at Phase 2 review and request approval before Phase 3.

## Acceptance criteria

The review report must cover all sections A–K in the supplied task. Every cleanup
has an identified boundary problem, inspected callers/diff, focused tests and a
coherent commit. Record commands, counts, skipped/ignored tests and actual
adapter classifications. Test CPU-only, WGPU-only, both and no-backend separately.
Run `just check`, `just python-test`, `just gen-effects-check`, `just docs-check`,
`just wgpu-software`, and strict `just wgpu-hardware` where available. Establish
canonical CPU performance with existing tooling, no concurrent heavy work and no
cache deletion. Report missing evidence explicitly; do not infer rendering or
performance equivalence from compilation alone.

Phase 3 acceptance will additionally require independent backend package builds,
isolated compilation evidence, packaging with fonts/shaders/new manifests,
public API/JSON/Python compatibility, complete frame/parity/lifecycle coverage,
controlled performance/memory comparisons, and CI/documentation migration.

## Progress and verification

2026-10-27: Read task and project guidance, inspected clean worktree/history,
fetched upstream and created the requested Phase 2 branch. Three independent
read-only audits are running. Baseline validation pending.

## Decisions and discoveries

2026-10-27: Explicit review gate belongs after Phase 2 cleanup and validation;
the original assignment does not authorize Phase 3 extraction.

## Completion / handoff

Pending. Preserve this plan and review report through the approval checkpoint.
Before eventual merge, transfer lasting architecture facts to permanent docs
and remove completed execution material according to [PLANS.md](../../../PLANS.md).
