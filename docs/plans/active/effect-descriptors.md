# Descriptor-driven effect authoring

Status: in progress
Branch: `refactor/effect-descriptors`
Baseline: `b23c985a623efea14511e25aeb1df93b495beecb` (fetched origin/develop).

## Objective

Generate repetitive Python effect declarations, stubs, factual reference tables
and synchronized JSON Schema from the existing core catalog, preserving every
public effect and its current behavior. Scope follows the user-provided
“Effect System Refactor: Phases 0 and 1” brief; no GitHub ticket was provided.

## Baseline and references

Phase 0 completed without tracked changes, branches or commits. Evidence and raw
logs: `/tmp/vestra-effects-baseline/README.md`. It records all 23 effects and 127
parameters, Python interfaces and behavior, 266 JSON cases, feature/dependency
checks, strict software and NVIDIA hardware validation, lossless frame references
and a ten-scenario canonical CPU benchmark. Fresh native 0.1.3 captures match.
Workspace: 1,130 Rust tests passed, 26 ignored; Python: 873 passed.

Read [effect development](../../development/extending/effect.md),
[effect pipeline](../../development/architecture/effect-pipeline.md),
[Python bindings](../../development/architecture/python-bindings.md), and
[testing](../../development/testing.md). Existing catalog/descriptors/property
constraints own canonical metadata; CLI already generates schema branches.

## Scope and non-goals

Extend the existing catalog with development-time Python metadata and sparse
reliable units. Generate mechanical declarations around existing adapters.
No renderer extraction, backend registry generation, execution/IR/shader changes,
new effects, dependencies, wire-format changes, push, merge or PR.

## Constraints and decisions

- Preserve existing native descriptor JSON through a separate development export.
- Keep one constructor signature per effect; derive all 21 typed helper signatures
  from it with four order overrides and four existing factory families.
- Keep runtime EffectDefinition unchanged; generation metadata stays separate.
- Move only pure visual schema generation into core, reused through an SDK export
  by CLI and directly by a core-only generator example. Preserve schema bytes.
- Preserve typed advanced validation timing, ownership, scalar/plain tracks,
  point/font/palette adapters, inheritance, slots, imports and PseudoAscii.
- Preserve the existing Ascii schema/parser omission mismatch explicitly.
- Use stdlib Python, bounded generated sections and lightweight nonmutating CI
  checks; retain handwritten narratives and specialized adapters.
- Controlled Rust checks use the documented CI debug=0 profiles and two jobs.
  Python native builds use maturin's release profile. Benchmark without strict
  WGPU requirement flags and without concurrent builds.

## Milestones

- [x] Capture exact-source baseline and document limits outside repository.
- [ ] Freeze compatible public API/behavior/JSON regression fixtures.
- [ ] Extend catalog and reuse pure schema generation without runtime changes.
- [ ] Generate Vignette/Glow pilots; prove signatures, behavior, animation,
      schema/native round trips and representative renders before expansion.
- [ ] Expand faithful generation across all effects; generate stubs/reference.
- [ ] Verify deterministic/idempotent outputs, invalid metadata diagnostics,
      stale-file CI detection and no writes during checks.
- [ ] Repeat baseline suites, lossless frames and benchmark; inspect differences.
- [ ] Finish focused commits, clean tree and detailed external final report.

## Acceptance criteria

All 23 effects retain API/default/validation/animation/JSON behavior. Generated
schema is byte-identical. Specialized exceptions are documented. Generator
requires core and Python stdlib only, fails clearly and preserves unchanged
files. Existing tests/tolerances remain intact; strict software/hardware checks
and baseline comparisons pass. Main/develop refs stay at the baseline commit.

## Progress and verification

2026-10-26: Phase 0 closed; branch created only after all baseline processes
finished. Default debug-symbol linking was impractical under host memory
pressure; controlled CI-profile checks passed. CodeGraph crashes; targeted
source inspection used. See external reports for exact commands/counts/timings.

## Decisions and discoveries

Python defaults differ from wire defaults. MotionTile/RadialBlur have no typed
advanced helper. Ascii enum defaults are optional in schema but required by
Rust parsing. Palette pass counts are conservative estimates, not fixed topology.
These are preserved contracts, not refactoring opportunities in this task.

## Completion / handoff

Implementation underway. Durable contributor instructions will live in the
existing effect extension guide. Temporary plan will be removed after final
verification; baseline and final evidence remain outside the repository.
