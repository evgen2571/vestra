# Documentation readability redesign
Status: in progress
Branch / baseline: `docs/readability-redesign` / `d2135907f4b2ce29a7cf3ee913dc50d9561ff26d`

## Objective
Make Contributing and Reference easy to enter, scan and use without changing runtime contracts.

## Baseline and references
The user request in this session is the specification (all five batches). Upstream `origin/develop` was fetched on 2026-11-07. The original checkout was clean; only one worktree existed. Coordinator: `/home/evgen2571/.codex/worktrees/f961/vestra`.
Read `AGENTS.md`, `PLANS.md`, documentation authoring, current implementation and tests. No CodeGraph index is available in this checkout; use focused source inspection.
Baseline `just docs-check`: 720 links, zero missing. `just site-check`: 14,346 references, zero errors; nine tests passed.

## Scope and non-goals
All contributor and reference pages, the linked historical benchmark, navigation, and effect-reference generator/template. No engine/API changes, new dependencies, media replacements, publication, push or PR.

## Constraints and decisions
- Preserve Python/canonical/schema/parser distinctions, error timing, units, coordinate/time domains, ownership, alpha, ordering and preparation requirements.
- Preserve incoming anchors with explicit spans where headings move. Preserve dated evidence, pinned revisions and licenses.
- Use the Text/Image pilots for reference structure; Testing and contributor landing for task routing. Adapt rather than mechanically standardize good pages.
- At most four total repository worktrees: original + coordinator + two workers. Workers receive separate branches and disjoint ownership after the pilot commit. Coordinator owns shared navigation and integration.
- Keep finished work local. Remove this completed plan before merge; retain the page inventory and verification handoff in ignored review artifacts and Git history.

## Milestones and ownership
- [x] Pilot (coordinator): Text, Image, contributor landing, Testing; source verification, docs/site checks and commit.
- [ ] Batch 1 (reference worker): remaining source pages; parameter groups and validation timing; docs-check.
- [ ] Batch 2 (contributor worker): GPU validation, four extension guides, targeted CONTRIBUTING updates; docs-check.
- [ ] Batch 3 (contributor worker + coordinator): all architecture/specialist pages; coordinator owns documentation splitting, Motion Tile relocation and algorithms; docs/site checks.
- [ ] Batch 4 (reference worker + coordinator): all other references; coordinator owns generated effect layout and tests; generation/docs/site checks.
- [ ] Batch 5 (coordinator): indexes/navigation, anchors, integrated review, desktop/mobile light/dark browser checks and local handoff.

## Acceptance criteria
Every scoped page is reviewed and improved or explicitly retained with a reason. Technical additions are checked against code/tests. Existing docs/site gates pass; generated documentation is fresh and generator tests pass. Changed executable examples use existing checks. Inspect built pages at desktop/mobile widths in both themes, including tables, code, diagrams, anchors, navigation and keyboard access. Review final baseline diff for unrelated changes and generated clutter. Commit all final work and leave branch/worktree intact.

## Progress and verification
2026-11-07: baseline checks passed (see above). Pilots complete. Pilot checks: `just docs-check` 736 links, zero missing; `just site-check` 14,436 generated references, zero errors; nine tests passed. Text/Image claims checked against source constructors, font resolution, values, lowering and high-level text tests. No executable rendering examples changed. Runtime/renderer tests are outside this documentation-only scope unless an executable example changes.

## Decisions and discoveries
2026-11-07: navigation already includes Animation and Effect parameters; the full reference catalog omits them. Contributor landing omits Effect algorithms, although navigation includes it. Preserve navigation's useful grouping.

## Page inventory
Status `pending` means not yet reviewed. Each owner must return per-page outcomes and source/check evidence.

| Page | Owner | Outcome / reason |
| --- | --- | --- |
| `CONTRIBUTING.md` | contributor worker | pending |
| `docs/benchmarks/vestra-vs-moviepy.md` | contributor worker | pending |
| `docs/development/README.md` | coordinator | improved: task chooser, separate agent route and algorithms link |
| `docs/development/architecture/cpu-renderer.md` | contributor worker | pending |
| `docs/development/architecture/crates.md` | contributor worker | pending |
| `docs/development/architecture/effect-pipeline.md` | contributor worker | pending |
| `docs/development/architecture/media-io.md` | contributor worker | pending |
| `docs/development/architecture/overview.md` | contributor worker | pending |
| `docs/development/architecture/project-to-plan.md` | contributor worker | pending |
| `docs/development/architecture/python-bindings.md` | contributor worker | pending |
| `docs/development/architecture/render-pipeline.md` | contributor worker | pending |
| `docs/development/architecture/wgpu-renderer.md` | contributor worker | pending |
| `docs/development/documentation.md` | coordinator | pending |
| `docs/development/effect-algorithms.md` | coordinator | pending |
| `docs/development/extending/backend.md` | contributor worker | pending |
| `docs/development/extending/effect.md` | contributor worker | pending |
| `docs/development/extending/source.md` | contributor worker | pending |
| `docs/development/extending/transition.md` | contributor worker | pending |
| `docs/development/gpu-validation.md` | contributor worker | pending |
| `docs/development/media-pipeline.md` | contributor worker | pending |
| `docs/development/performance.md` | contributor worker | pending |
| `docs/development/releasing.md` | contributor worker | pending |
| `docs/development/stylization/dithered-palette-look.md` | contributor worker | pending |
| `docs/development/stylization/temporal-diagnostics.md` | contributor worker | pending |
| `docs/development/testing.md` | coordinator | improved: change/check chooser, focused/pre-PR/equipment routes and named compatibility/CI sections |
| `docs/reference/animation.md` | reference worker | pending |
| `docs/reference/audio.md` | reference worker | pending |
| `docs/reference/backends.md` | reference worker | pending |
| `docs/reference/catalog.md` | coordinator | pending |
| `docs/reference/cli.md` | reference worker | pending |
| `docs/reference/diagnostics.md` | reference worker | pending |
| `docs/reference/effect-parameters.md` | coordinator | pending |
| `docs/reference/effects.md` | reference worker | pending |
| `docs/reference/environment-variables.md` | reference worker | pending |
| `docs/reference/feature-support.md` | reference worker | pending |
| `docs/reference/index.md` | coordinator | pending |
| `docs/reference/masks.md` | reference worker | pending |
| `docs/reference/nested-compositions.md` | reference worker | pending |
| `docs/reference/presets-and-flashes.md` | reference worker | pending |
| `docs/reference/project-format.md` | reference worker | pending |
| `docs/reference/python-api.md` | reference worker | pending |
| `docs/reference/rust-sdk.md` | reference worker | pending |
| `docs/reference/signals.md` | reference worker | pending |
| `docs/reference/sources/image.md` | coordinator | improved: sizing, crop, paths, presentation and validation sections |
| `docs/reference/sources/particle-system.md` | reference worker | pending |
| `docs/reference/sources/shape.md` | reference worker | pending |
| `docs/reference/sources/solid-color.md` | reference worker | pending |
| `docs/reference/sources/spectrum2d.md` | reference worker | pending |
| `docs/reference/sources/text.md` | coordinator | improved: labelled signatures, parameter/font tables, error stages and canonical fields |
| `docs/reference/sources/video.md` | reference worker | pending |
| `docs/reference/transitions.md` | reference worker | pending |

## Completion / handoff
In progress. Final handoff must record branch, worktree, baseline/final revisions, batch outcomes, retained pages, actual checks, unavailable checks, and no push/PR/deployment.
