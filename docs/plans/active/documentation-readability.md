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
- [x] Batch 1 (reference worker): remaining source pages; parameter groups and validation timing; docs-check.
- [x] Batch 2 (contributor worker): GPU validation, four extension guides, targeted CONTRIBUTING updates; docs-check.
- [x] Batch 3 (contributor worker + coordinator): all architecture/specialist pages; coordinator owns documentation splitting, Motion Tile relocation and algorithms; docs/site checks.
- [x] Batch 4 (reference worker + coordinator): all other references; coordinator owns generated effect layout and tests; generation/docs/site checks.
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
| `CONTRIBUTING.md` | contributor worker | Improved: Kept its existing numbered contribution flow; added a task-map route and made documentation/site checks distinct from the native contributor gate. |
| `docs/benchmarks/vestra-vs-moviepy.md` | contributor worker | Improved: Routes new work to current methodology and labels optimization priorities as historical proposals. Preserved every measurement table, run date, revision, reproduction command, raw-evidence path, limitation and source-input provenance. |
| `docs/development/README.md` | coordinator | improved: task chooser, separate agent route and algorithms link |
| `docs/development/architecture/cpu-renderer.md` | contributor worker | Improved: Sectioned preparation, execution, ownership/caching, fast paths/metrics and memory accounting. Separated current concurrency policy, current limits and deferred admission. All numerical estimates, alpha/sampling constraints, ownership and exclusions retained. |
| `docs/development/architecture/crates.md` | contributor worker | Retained: Already has a short entry, dependency diagram, responsibility table and clear dependency/lifetime rules. Another rewrite would add uniformity without helping navigation. |
| `docs/development/architecture/effect-pipeline.md` | contributor worker | Retained: Already separates flow/ownership, ordering/spaces, resource bounds, numerical contracts, stylization requirements and verification. Detailed implementation route belongs in the extension guide; no extra inventory needed. |
| `docs/development/architecture/media-io.md` | contributor worker | Improved: Added headings for paths/preparation and frame writes/publication to expose the existing lifecycle. Kept native FFmpeg versus executable dependencies and publication error context. |
| `docs/development/architecture/overview.md` | contributor worker | Retained: Nineteen-line overview already gives ownership, a flow diagram, validation/preflight boundary and explicit next-page links. It serves as an entry rather than an exhaustive reference. |
| `docs/development/architecture/project-to-plan.md` | contributor worker | Retained: Already has flow and source maps, time-domain table, checked rounding rules and duration/validation authority. It explicitly distinguishes current APIs from proposals; retain these exact boundary explanations. |
| `docs/development/architecture/python-bindings.md` | contributor worker | Improved: Added native/prepared-state, callback/logging/error, and source/font-lowering sections plus change/reference routes. Kept GIL, exception/cleanup, immutable snapshot, font and group semantics. |
| `docs/development/architecture/render-pipeline.md` | contributor worker | Improved: Added prepared-state/frame ownership, layer ordering, and terminal outcome headings; linked public lifecycle and encoded-output verification. Preserved validation/preflight timing, ordering and publication success boundary. |
| `docs/development/architecture/wgpu-renderer.md` | contributor worker | Improved: Exposed preparation, execution, canvas/mask precision, resource ownership/reuse, module map, caching/capabilities and metrics/limits. Clarified acquisition of the existing shared root instance. Kept device lifetimes, rounding, allocation and metric caveats. |
| `docs/development/documentation-media.md` | coordinator | added: extracted reproduction recipes and provenance unchanged |
| `docs/development/documentation-site.md` | coordinator | added: extracted site maintenance, browser verification and authorized publication procedures |
| `docs/development/documentation.md` | coordinator | improved: editing/writing/checking focus; reference conventions and compatible links to specialist workflows |
| `docs/development/effect-algorithms.md` | coordinator | improved: palette/CRT/ASCII subsections and reused processing diagram; numerical contracts retained |
| `docs/development/extending/backend.md` | contributor worker | Improved: Linked current lifecycle and CPU/WGPU implementations; added file/responsibility map, ordered route, lifecycle/fallback/isolation/media checks and completion criteria. Preserved SDK/media ownership and truthful support reporting. |
| `docs/development/extending/effect.md` | contributor worker | Improved: Retained its strong detailed route and existing linked implementations; added concise route/file-map orientation and a completion condition. Kept catalog, compile/evaluate, resources, generated-surface and ABI/pixel distinctions intact. |
| `docs/development/extending/source.md` | contributor worker | Improved: Linked real Spectrum2D and asset-backed model examples; added ownership map and ordered model/compiler/preparation/backend/Python route. Kept source-specific attachment capabilities and local timing explicit. |
| `docs/development/extending/transition.md` | contributor worker | Improved: Linked Python definitions, mapped owners and explained reuse of generic channels before core extension. Added ordered implementation, nested-timing tests and readiness criteria without inventing a new canonical kind. |
| `docs/development/gpu-validation.md` | contributor worker | Improved: Separate adapter discovery, software-safe correctness, strict hardware correctness, evidence interpretation, platform troubleshooting and lifecycle stress. Corrected stress text for GL's actual one-worker restriction; hardware/software evidence remains separate. |
| `docs/development/media-pipeline.md` | contributor worker | Improved: Added task routing and a probe/frame-AV/duration-failure verification sequence. Preserved PTS/VFR selection, decode thresholds and failures, CPU/WGPU ownership, bounded overlap, AV-sync and publication/cleanup checks. |
| `docs/development/performance.md` | contributor worker | Improved: Leads with baseline/candidate comparison. Added focused investigation chooser with prerequisites/output, exact stage-isolation report names and an explicit existing native-worker invocation. Retained paired-run methodology, thresholds, profiling limits and private fixture requirements. |
| `docs/development/releasing.md` | contributor worker | Improved: Added checklist-first entry and sections for version/artifact contract, publication controls and artifact attachment order. Retained all matrix, installed-wheel/sdist, license/source-archive and approval checks. |
| `docs/development/stylization/dithered-palette-look.md` | contributor worker | Improved: Distinguished the design target from current controls and supplied an ordered regression workflow. Kept three-palette acceptance, alpha/time checks, pinned example revision and AcerolaFX provenance/license. |
| `docs/development/stylization/temporal-diagnostics.md` | contributor worker | Improved: Sectioned current fixture generation, interpretation and full-resolution checks. Labeled 2026-10-10 acceptance evidence as historical; retained deferred rows. Corrected requested-API artifact wording: same-API runs can overwrite evidence and must be archived separately. |
| `docs/development/testing.md` | coordinator | improved: change/check chooser, focused/pre-PR/equipment routes and named compatibility/CI sections |
| `docs/reference/animation.md` | reference worker | Improved lightly: headings separate clocks/segment rules, named modes and custom Bézier curves. Python and JSON examples unchanged. |
| `docs/reference/audio.md` | reference worker | Improved lightly: timeline/track/clip inventory table and canonical-field link. Timing, effect, crossfade and output sections retained. |
| `docs/reference/backends.md` | reference worker | Retained: already clearly separates requested backend, selected renderer and adapter, uses a compact preference table and isolates hardware/parity limits. Rewriting for uniformity would add no reader value. |
| `docs/reference/catalog.md` | coordinator | improved: complete grouped topic list with descriptions; old anchors retained |
| `docs/reference/cli.md` | reference worker | Improved lightly: results/diagnostics/logging and progress-stream rules now have headings. Existing command/render-option/validation tables retained. |
| `docs/reference/diagnostics.md` | reference worker | Improved lightly: headings expose video timing pointers, distinction from logs/progress, and code stability. |
| `docs/reference/effect-parameters.md` | coordinator | improved via generator: four-column authoring tables and expandable exact JSON/planner metadata; all effect anchors retained |
| `docs/reference/effects.md` | coordinator | Improved: Motion Tile anchor regression relocated beside its contract; other effective sections retained. |
| `docs/reference/environment-variables.md` | reference worker | Improved lightly: explicitly separates supported user settings from internal tuning/verification controls, fixing the misleading opening assertion that such controls were excluded. |
| `docs/reference/feature-support.md` | reference worker | Improved: grouped source and mask status tables; concise effect/transition/audio statuses separated from scope and focused verification evidence. Replaced redundant effect-control inventories with authoritative parameter-reference link. Preserved CPU/software/hardware distinctions, native-Windows limitation, deferred controls and pinned provenance. |
| `docs/reference/index.md` | coordinator | improved: task-to-guide/reference route and links to exact animation/effect parameters |
| `docs/reference/masks.md` | reference worker | Improved: owned-mask/matte comparison, field/default table and separate operations, inputs, intrinsic coordinates, video timing, owned groups, feather ordering and matte lifetime/dependencies. Preserved pinned examples and exact formulas. |
| `docs/reference/nested-compositions.md` | reference worker | Improved: separated placement/local clock, transitions, size/coordinates, clipping/fractional raster bounds, child effects/masks, resource validation and compatibility. |
| `docs/reference/presets-and-flashes.md` | reference worker | Improved: separate signatures and parameter tables for Image-only presets and root flashes; explicit construction errors and clock domains. |
| `docs/reference/project-format.md` | reference worker | Improved: explicit headings for Project, Output, Asset, Visual, Clip, Mask, Track, Transform, Group, Binding and Audio objects. Validation layers precede separate budget categories. Clarified normalized transform position. Legacy heading anchors retained as spans. |
| `docs/reference/python-api.md` | reference worker | Improved: separate authoring, immutable snapshots, preparation/rendering, resources, progress/cancellation, logging and errors. Labelled signatures replace inline inventories and the long runtime table. Preserved source ownership, semantic/preflight distinctions, callback failures and publication timing. |
| `docs/reference/rust-sdk.md` | reference worker | Improved lightly: progress/cancellation entry points and JSON effect authoring have distinct headings. Existing examples, error/resource/backend contracts retained. |
| `docs/reference/signals.md` | reference worker | Improved lightly: headings separate feature selection, ordered transforms and binding/time domain. |
| `docs/reference/sources/image.md` | coordinator | improved: sizing, crop, paths, presentation and validation sections |
| `docs/reference/sources/particle-system.md` | reference worker | Improved: grouped emitters, emission/lifetime, motion and appearance tables; normalized units, rotation naming, range validation stages, canonical nesting and preparation. |
| `docs/reference/sources/shape.md` | reference worker | Improved: labelled geometry/style signatures, grouped fill/stroke rules and pixel geometry, construction/preparation/support sections. |
| `docs/reference/sources/solid-color.md` | reference worker | Improved: source signature/default and validation; canvas extent separated from Python adapter presentation and canonical transform/transition restrictions. |
| `docs/reference/sources/spectrum2d.md` | reference worker | Improved: analysis/response separated from normalized layout/color; labelled layouts, exact rectangle/gap/sensitivity constraints and validation/preparation stages; preset and color-name lowering. |
| `docs/reference/sources/text.md` | coordinator | improved: labelled signatures, parameter/font tables, error stages and canonical fields |
| `docs/reference/sources/video.md` | reference worker | Improved: construction/placement signatures, parameter tables, immediate probing versus later preflight, canonical clip timing and media errors. Corrected omitted sizing to intrinsic dimensions after checking compiler defaults. |
| `docs/reference/transitions.md` | reference worker | Improved lightly: endpoint sources and nested ownership now have a named subsection. Existing placement, definition, normalized channel and validation sections already work. |

2026-11-07: coordinator specialist work: generation freshness zero changes; six metadata-exporter tests and 20 generator tests passed. Generator layout tests first failed against the old layout, then passed; escaping checked in authoring and canonical tables. Ruff check/format passed after formatting. Site/catalog regression first demonstrated missing Animation/Effect parameters; now passes. `just docs-check`: 751 links, zero missing; `just site-check`: 14,962 references, zero errors; ten tests passed. A strict-build failure exposed a legacy catalog anchor and was fixed with a compatibility span (no checks weakened).

2026-11-07: both workers complete; coordinator reviewed every diff and inspected changed timing/unit/default claims against source. All 41 worker pages reviewed: 36 improved, five retained (architecture overview, crates, project-to-plan, effect-pipeline, and reference backends). Integrated commits: 3088aa8, 26a34a8, be9159e, 70c985e, 248d4a1, 2b8f85f, aaf61d8. Most recent combined checks: `just docs-check` 797 links, zero missing; `just site-check` 15,455 references, zero errors, ten tests passed. Baseline-site heading audit: no lost anchors. Executable scene blocks unchanged in worker scopes. Corrected the pilot assumption that omitted Image/Video sizing inherited a default: compiler/lowering prove Original intrinsic sizing. No runtime changed.

Worker reports copied to ignored `target/readability-review/`. Native snippet tests, integrated browser inspection and final independent review remain underway.

## Completion / handoff
In progress. Final handoff must record branch, worktree, baseline/final revisions, batch outcomes, retained pages, actual checks, unavailable checks, and no push/PR/deployment.
