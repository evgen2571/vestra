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
- Derive constructor annotations and defaults from canonical structured descriptors;
  maintain only Python layout/authoring overrides in developer tooling, with four
  helper order overrides and four existing factory families. No full signature strings.
- Keep runtime EffectDefinition and public core API unchanged. Consume the same
  catalog_entries.rs input from the runtime macro and development example;
  Python-specific overrides stay in scripts/effects/python_metadata.py.
- Share the existing visual schema builder under developer tooling between CLI
  and the core-only exporter. Do not add a runtime core schema module or SDK API.
  Preserve schema bytes and avoid a new tool crate/dependency.
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
- [x] Freeze compatible public API/behavior/JSON regression fixtures.
- [x] Extend catalog and reuse pure schema generation without runtime changes.
- [x] Generate Vignette/Glow/OrderedDither pilots; prove signatures, behavior, animation,
      schema/native round trips and representative renders before expansion.
- [x] Expand faithful generation across all effects; generate stubs/reference.
- [x] Verify deterministic/idempotent outputs, invalid metadata diagnostics,
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

2026-10-26 user clarification: 23 canonical effects are correct. Two distinct
24-counts exist: the catalog includes internal ColourTransform, and Python adds
PseudoAscii as an authoring adapter for ascii. Preserve all 23 canonical effects,
21 typed helpers, the generic path for all 23, and the alias. OrderedDither adds
inheritance, keyword defaults, animation and palette-adapter coverage beyond
Glow's runtime multi-pass complexity. Keep schema source in shared developer
tooling; catalog metadata remains in core, with no runtime parsing or hot-path
layout changes. Compatibility fixtures are repository-contained, not temporary
log-dependent. Initial focused Vignette/Glow tests: 190 passed, 1 schema-freshness
test deliberately deselected while the Rust worker owned Cargo; schema byte
parity was verified separately. All-catalog compatibility tests initially 328
passed before expansion; final verification remains pending.

## Completion / handoff

Implementation underway. Durable contributor instructions will live in the
existing effect extension guide. Temporary plan will be removed after final
verification; baseline and final evidence remain outside the repository.

2026-10-26 approved clarification: removed the draft public core generation API
and opaque Python signature strings. Core mod/descriptors remain baseline-identical.
The dev exporter consumes the same semantic catalog invocation; Python metadata
is a structured development-only overlay keyed by canonical id. All 23 generated
constructor ASTs match the captured signatures with derived annotations/defaults.
No new crate or runtime feature was introduced. Pilot captures: 60 RGBA files
byte-identical; software stylization 36 passed/11 ignored. Hardware pilot gate
and final native rebuild remain pending.

2026-10-26 catalog expansion complete: 23 canonical classes, 21 existing helpers,
eight new stubs and generated parameter reference. Frozen API/authoring/typing
checks: 541 passed, one Cargo-based schema check left to full gate. Generator:
12 stdlib tests passed; Style passed; 19 artifacts unchanged across repeated
regeneration/check with preserved mtimes. Core/example/CLI focused checks pass
and schema bytes remain baseline-identical. Strict hardware pilot: 36 passed,
11 ignored; targeted OrderedDither original/generated JSON and 12 rendered
comparisons exact, six CPU/software comparisons exact, contact sheet inspected.

A real isolated PaletteMap levels default4->8 edit propagated to generated class,
helper, stub, schema and reference with no Python default override or shipped
source change. Evidence: /tmp/vestra-effects-after/propagation/README.md.

Fresh locked release native rebuild completed in146.03s; extension version0.1.3,
SHA256 be8c2ae7fbf66ce5b3c4eca4cc5f33bf4dbc79c8c8b790ae40e27d02fc34a4a4
is byte-identical to baseline. Full Python/Rust/software/hardware and benchmark
verification is in progress under one Cargo owner.
