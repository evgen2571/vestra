# Effect architecture implementation plan

Status: in progress
Branch / baseline: `refactor/effect-architecture` / `8fb4847f6d2b85d5a09d09179b638a39652dff78`

## Objective

Implement the attached Phase 4 specification: a predictable extension path
whose maintained edits describe the effect, its semantics and its algorithms.
Preserve public Rust/Python/JSON contracts, pixels, diagnostics and resources.
The full specification is the user attachment `pasted-text-1.txt` in this
session; its eight milestones and deliverables govern this plan.

## Baseline and references

Fetched `origin/develop` on 2026-10-28. The checkout was clean; the preferred
branch did not exist and was created from the fetched head. No protected
branch modifications, push, PR or merge are authorized.

Read [contributing](../../../CONTRIBUTING.md), [plan policy](../../../PLANS.md),
[effect guide](../../development/extending/effect.md),
[effect pipeline](../../development/architecture/effect-pipeline.md),
[testing](../../development/testing.md), [GPU validation](../../development/gpu-validation.md)
and relevant repository skills. CodeGraph exits 139, so use direct inspection.

The current code has 23 public effects / 24 catalog kinds, 20 logical operations,
19 renderer kernels, 19 WGPU parameter variants and 13 record layouts.
CPU/WGPU support functions return unconditional true. Nine actual base GPU
pipelines plus 19 effect pipelines produce 28 pipelines, while constants report
27. Preserve and characterize this discrepancy; correcting statistics is outside
the mechanical migration. The size inventory omits Analog, Ascii and
InputAnalysis records. Existing Python generation owns 19 artifact files;
advanced typed classes and exports still require handwritten integration.

## Scope and non-goals

Core declarations, effect-local semantics, common kernel registration,
CPU/WGPU registration, Python generation, completeness checks, extension
exercises, documentation and full validation are in scope. No runtime plugin
system, source scaffolding command, changed effect algorithms, media ownership
changes, new public effects or unrelated statistics corrections.

## Constraints and decisions

- Keep the four renderer crates and core/media dependency boundaries.
- Use cooperating Rust declaration macros, building on the catalog. A procedural
  macro or development generator is justified only if ordinary macros cannot
  express the needed projections clearly. No downstream Python build step.
- Authoritative effect declarations carry authored/compiled/evaluated fields,
  existing serde attributes, property associations and semantic module identity.
  Generate whole enums and mechanical visitors/forwarding, keeping current public
  paths and construction/pattern shapes.
- Move validation, compile/evaluate, identity, dependency and pass decisions into
  effect-local Rust modules. Preserve diagnostic ordering and legacy differences
  in transition traversal and normalization rather than broadening behavior.
- Preserve `ColourTransform` as internal-only; composites reuse operations;
  transform effects have no fabricated processing kernel.
- Common declaration owns kernel order and explicit private bits independently.
  Backend-local declarations require exhaustive, deliberate support decisions.
- WGPU declaration owns exact shader fragments/labels and typed parameter/upload
  association. Record fields, packing, resource lifecycle and mismatch guard stay
  explicit. Structural ABI checks cover every registered record automatically.
- Python metadata retains constructor ergonomics and historical export layout;
  derive typed classes, imports and mechanical associations from catalog facts.
- Two worktrees maximum: lead checkout and `/tmp/vestra-effect-renderer`.
  Renderer worker owns only renderer crates/tests; Python worker owns generator,
  Python declarations/exports and generator tests. Lead owns core, integration,
  plan, documentation and final validation. Integrate controlled commits.

## Milestones

- [ ] 0. Freeze compatibility evidence before migration: core API/serde,
  compile/evaluate/pass/dependency behavior; renderer order/bits/shader bytes,
  ABI/upload fingerprints; existing Python frozen fixtures. Capture baseline
  rendering and comparable performance before production changes.
- [ ] 1. Implement Vignette, Glow and Camera Shake pilots, verify them, then
  migrate all core effects and remove old mechanical integration. Check all
  property descriptors against visitors and semantic paths.
- [x] 2. Consolidate common kernels and deliberate CPU/WGPU capabilities.
- [x] 3. Consolidate WGPU shaders/parameters/uploads and automatic ABI checks.
- [x] 4. Consolidate CPU registration without execution lifecycle changes.
- [ ] 5. Complete Python generation and exhaustive/structural checks; preserve
  existing `gen-effects` commands and frozen compatibility fixtures.
- [ ] 6. Exercise temporary ordinary FilmGrain-style addition through every
  layer, remove it, and record actual file/site/generated-file counts. Exercise
  composite and transform cases with no artificial kernel registrations.
- [ ] 7. Publish practical `docs/development/adding-effects.md`, update existing
  architecture/testing links and execute final validation. Commit all changes
  on the requested branch, with no push, merge or PR.

## Acceptance criteria

Run focused tests per milestone; then formatting, workspace build/check,
warnings-denied Clippy, unit/integration/doctests/examples, isolated CPU/WGPU/no
backend selections and dependency graphs. Verify Python frozen interfaces,
canonical serialization/schema/freshness, full tests, native import, wheel smoke
and clean sdist installation. Execute strict software WGPU and strict hardware
verification when discovery proves a real adapter; record adapter/API/driver.
Inspect actual rendered frames and compare representative resource/performance
workloads using equivalent environments. Distinguish compile-time exhaustiveness,
structural checks, specimen coverage, pixels and hardware evidence.

Review focus: serde flatten/default/unknown-field behavior; scalar/plain/point
visitor completeness; effect-local timing and diagnostic order; composite
secondary/original surface liveness; typed Rust/WGSL ABI and upload identity.

## Progress and verification

- Baseline `cargo test -p vestra-core`: 344 unit tests passed, one ignored;
  one integration test passed; zero doctests.
- Renderer baseline measurements and characterization tests are being captured
  in the worker worktree before its registration changes.
- Python worker is running the frozen pre-generation compatibility tests before
  expanding generation; frozen fixtures will not be refreshed.
- Hardware, pixels, packaging, performance and integrated validation pending.
- Core representations now expand from `catalog_entries.rs`; enums remain in
  their original defining modules and public property enum discriminants are
  pinned explicitly. All 345 unit tests, the integration test, six exporter
  tests, frozen specimen hashes and warnings-denied core Clippy passed.
- Vignette/Glow/Camera Shake semantic pilots pass core tests and all 175 CPU
  tests. Full migration follows after this independently verified pilot.
- Renderer commits integrated through `c8ab41d`. Worker validation: common
  64 passed/3 ignored, CPU 175 passed/6 ignored, WGPU 150 passed/4 ignored;
  all three crates Clippy all-targets with warnings denied and formatting pass.
  Strict WGPU used actual GTX 1650 SUPER / Vulkan / NVIDIA 595.71.05.
  Mutation probes reject duplicate kernel bits, missing CPU/WGPU rows and
  swapped same-type uniform fields. These are structural/compile checks,
  separate from representative packed-value and rendered-pixel checks.
- Before-render artifacts secured under ignored
  `target/effect-architecture-baseline/`: exact baseline Python/native package,
  80 frame requests across ten scenes on each CPU/software/hardware backend,
  repeated/backward-request assertions, raw RGBA/PNG/hashes and timing samples.
  Use `before-fit` and `before-hardware-fit`; initial natural-size captures
  were superseded because Vignette did not visibly affect that small source.
- Hardware setup: in `nix develop .#wgpu-software`, unset
  `VESTRA_WGPU_FORCE_FALLBACK` and set
  `VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json`.
  Software shell uses llvmpipe/Mesa 26.1.5; never mix its timings with hardware.

## Decisions and discoveries

- 2026-10-28: Native cargo works for pure core/renderer crates. Use the pinned
  Nix environment for workspace/Python/media and adapter validation as needed.
- 2026-10-28: Existing transition/normalization visitors intentionally need exact
  characterization: palette input fields are absent from some legacy visitors.
  Do not change those semantics during mechanical generation.

## Completion / handoff

Not complete. All milestone checkboxes require evidence, not implementation intent.
Retain this active plan during local branch work; transfer lasting knowledge to
permanent docs and remove the completed plan before a future authorized merge.
