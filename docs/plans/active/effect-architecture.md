# Effect architecture implementation plan

Status: in progress — final hardening and integration
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

- [x] 0. Freeze compatibility evidence before migration: core API/serde,
  compile/evaluate/pass/dependency behavior; renderer order/bits/shader bytes,
  ABI/upload fingerprints; existing Python frozen fixtures. Capture baseline
  rendering and comparable performance before production changes.
- [x] 1. Implement Vignette, Glow and Camera Shake pilots, verify them, then
  migrate all core effects and remove old mechanical integration. Check all
  property descriptors against visitors and semantic paths.
- [x] 2. Consolidate common kernels and deliberate CPU/WGPU capabilities.
- [x] 3. Consolidate WGPU shaders/parameters/uploads and automatic ABI checks.
- [x] 4. Consolidate CPU registration without execution lifecycle changes.
- [x] 5. Complete Python generation and exhaustive/structural checks; preserve
  existing `gen-effects` commands and frozen compatibility fixtures.
- [x] 6. Exercise temporary ordinary FilmGrain-style addition through every
  layer, remove it, and record actual file/site/generated-file counts. Exercise
  composite and transform cases with no artificial kernel registrations.
- [x] 7. Publish practical `docs/development/adding-effects.md`, update existing
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

Implementation is committed through `cfe04bc97554d069118cc69b8a082c909304bdcb`.
The final documentation and acceptance results are committed with this plan.

### Architecture and compatibility evidence

- Core pilots (Vignette, Glow, Camera Shake) passed before full migration.
  All three representations now derive from the catalog in their original
  defining modules. All effect semantics are local; old central semantic
  matches and manual property/identity/classification inventories are removed.
- Optional Assets/Transform/Motion hooks replace the old special evaluator
  cases. Basic-colour composition derives from the existing class. Default
  semantic policies derive directly; 84 constant/catalog forwarding functions
  were removed. Declared override hooks still require implementations.
- Frozen original-revision specimens cover all 23 existing effects, both static
  and animated, with serde, compile/evaluate, identity, dependencies, properties
  and topology. Separate 46-specimen normalization/remapping capture preserves
  the ten historical palette input exclusions. Neither fixture was regenerated
  from the replacement architecture.
- Renderer specimens pin common kernel order/private bits, exact shader labels
  and composed bytes, and representative packed records. Registry-driven ABI
  checks cover all 13 record layouts automatically, including previously omitted
  Analog/Ascii/InputAnalysis. Named offsets, scalar kinds, layout and typed
  upload are structural checks, not proofs of every packed numerical value.
- Actual mutation probes reject duplicate kernel bits, missing CPU/WGPU rows,
  duplicate scalar discriminants, missing transform functions and wrong uniform
  scalar kinds/offsets. Two qualified/attributed-field regressions were observed
  failing before the final fix and passing afterwards. Arbitrary type aliases
  remain outside the documented typed-field grammar.
- Final independent source review found no confirmed important production bug.
  It identified missing scratch conformance specimens; those were added and
  all three full trial recipes now pass.

### Final native and rendering checks

The final Rust source revision `cfe04bc` was force-rebuilt with
`uv sync --locked --extra dev --reinstall-package vestra`. Loaded native SHA256:
`adc5ea784da3602de3798cc29fb5167819d2548cf874953732b7271e5ec216ef`.
Ordinary uv synchronization had reused an old extension; explicit reinstall is
now documented for Rust-only changes.

| Check against final Rust source | Observed result |
| --- | --- |
| CPU original/candidate frame requests | 80/80 byte-identical |
| llvmpipe Vulkan original/candidate frame requests | 80/80 byte-identical |
| NVIDIA Vulkan original/candidate frame requests | 80/80 byte-identical |
| Canonical project values | 30/30 identical |
| Full Python suite, native import, compileall | 1,205 passed; import/compileall passed |
| Fresh wheel build/install and installed audio/glyph smoke | Passed |
| Source archive/current included source comparison | 454 files identical, including 26 semantic modules |
| Fresh sdist installation outside checkout with isolated Cargo target | Passed; installed audio/glyph smoke passed |
| Rust/Python formatting and maintained Python lint | Passed |
| Documentation links | 339 checked, zero missing |

Rendered scenes include source, Vignette, Glow, Gaussian, palette/dither, ASCII,
Camera Shake, Sharpen and chains, including repeated/backward frame requests.
The final contact sheet was inspected. Exact requests, canonical values, RGBA,
PNGs, native hashes and command logs remain under
`target/effect-architecture-baseline/`; final-native summary is
`cfe04bc-native-status.json`, pixels are `cfe04bc-comparison.json`.
Packaging evidence is under `target/effect-architecture-packaging/cfe04bc/`.

The uncached sdist build-requirement download stalled while PyPI was unreachable;
that attempt was terminated and recorded. The successful retry used cached
build requirements with `uv --offline`, the same current archive, a fresh venv,
an external working directory and an isolated Cargo target. Current engine
source was compiled; a previous installed package was not reused.

### Rust, adapters and isolation

Final production checks passed in `nix develop .#wgpu-software` with
`CARGO_BUILD_JOBS=2`:

| Command | Observed result |
| --- | --- |
| `just check` | Passed: formatting, all-features workspace check, all-targets warnings-denied Clippy, 1,157 workspace Rust tests, doctest targets, six exporter/schema tests, 19 generator tests, schema and docs freshness |
| `cargo build --locked --workspace --all-features` | Passed |
| `cargo build --locked --workspace --all-features --examples` | Passed |
| `just effect-check` | Passed, including 122 focused WGPU library tests |
| `just wgpu-software` | Passed on llvmpipe Vulkan, including workspace and strict adapter-dependent tests |
| `just wgpu-hardware` | Passed on GTX 1650 SUPER Vulkan, including workspace, strict renderer tests and the CLI encoded-frame regression |

Thirty existing ignored Rust tests remain unexecuted: manual benchmarks,
concurrent lifecycle stress, exhaustive RGB packing and explicit 1080p/4K
correctness/resource suites. No tests were newly ignored and no assertions were
weakened. Other operating systems, graphics APIs and GPU vendors remain
unverified. All three feature-isolated facade tests passed in separate fresh
Cargo target directories: backend-free four tests, CPU-only four tests, WGPU-only two tests.
Compiler artifact inventories contain 139, 140 and 177 targets respectively.
CPU-only has its CPU implementation and no WGPU/Naga artifact; WGPU-only has
its WGPU implementation and no CPU artifact; backend-free has neither. Presence
as well as exclusion was asserted. Commands use `cargo test --locked -p
vestra-render --no-default-features`, adding `--features cpu` or `wgpu` and
`--message-format=json-render-diagnostics`. Isolated debug symbols are disabled;
assertions remain enabled. Exact commands, environments, exit codes, compiler
JSON and artifact inventories are under
`target/effect-architecture-validation/`; `final-rust-results.json` records all
13 final command stages, including four timing runs, with exit code zero.

Dependency graph checks pass for CPU-only, WGPU-only, backend-free, media and
core selections. CPU selects no WGPU/Naga package; WGPU selects no CPU
implementation; core/media select no renderer. Logs and assertions are under
`target/effect-architecture-validation/`. Isolated compiler artifacts are
collected in addition to these graphs.

Actual hardware is NVIDIA GeForce GTX 1650 SUPER, discrete GPU, Vulkan,
NVIDIA driver 595.71.05. Software is llvmpipe, Mesa 26.1.5 / LLVM 21.1.8.
The default pinned software shell discovers llvmpipe; hardware runs must unset
`VESTRA_WGPU_FORCE_FALLBACK` and use
`VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json`.
Desktop GL 3.3 was unusable and is not a substitute for Vulkan validation.
Toolchain: Rust/Cargo 1.95.0, FFmpeg 8.1.2, Python 3.13.14, Linux/Nix.

### Performance and resource limits

After all compilation finished, final-source ABBA timings used the frozen
original native library and the rebuilt `cfe04bc` library, CPU 640x360, 24 warmup
frames, seven samples of eight frame requests. Order was original A3, candidate
B3, candidate B4, original A4; native hashes are recorded in every JSON file.

| Workload | Original A3 / A4 medians, ms/frame | Final B3 / B4 medians, ms/frame |
| --- | ---: | ---: |
| vignette | 30.20, 27.68 | 25.14, 35.58 |
| glow | 37.98, 38.68 | 39.11, 47.92 |
| gaussian_blur | 33.54, 37.98 | 34.28, 42.43 |
| chain | 53.42, 48.55 | 51.07, 54.91 |

Run-to-run variation remains substantial, including within the same candidate.
Earlier paired/ABBA samples also drifted and initial apparent slowdowns did not
repeat. The measurements do not justify a performance-neutrality or speedup
claim; they cannot exclude a material regression. Raw distributions and earlier
CPU/NVIDIA timings remain under `target/effect-architecture-baseline/`.

The refactor preserves explicit algorithm/packing/resource work and static
forwarding. The existing pipeline-statistics discrepancy remains characterized:
28 actual pipeline creations versus 27 reported. This unrelated counter bug was
not silently corrected. Structural tests and pixel specimens supplement actual
resource/fallback tests; none establish every numerical combination or device.

## Decisions and discoveries

- 2026-10-28: Native cargo works for pure core/renderer crates. Use the pinned
  Nix environment for workspace/Python/media and adapter validation as needed.
- 2026-10-28: Existing transition/normalization visitors intentionally need exact
  characterization: palette input fields are absent from some legacy visitors.
  Do not change those semantics during mechanical generation.
- 2026-10-28: Resumed final acceptance at `cfe04bc`. No old validation jobs
  remained. Serialize native, scratch-trial and workspace builds with two Cargo
  jobs; read-only review and documentation may proceed concurrently.
- Final dependency graphs pass for CPU-only, WGPU-only, backend-free, media and
  core selections. CPU selects no WGPU/Naga package; WGPU selects no CPU
  implementation; core/media select no renderer. Logs are under
  `target/effect-architecture-validation/dependencies-*.txt`. Final isolated compiler artifacts
  additionally prove these backend exclusions.
- Documentation link check: 339 local links in 95 Markdown files, zero missing.

## Workflow measurements

The actual temporary implementations and their complete patches/path inventories
are retained in `target/effect-extension-trials/`. They are not shipped effects.
"Maintained files" excludes generated files, tests and documentation. A manual
registration site is an inventory, association, visitor or import/export list;
algorithm bodies, validation, packing and semantic topology are implementation,
not additional registration sites. Fields within one declaration count once.

| Actual post-refactor trial | Maintained files | Registration sites | Generated files changed | Test-only files |
| --- | ---: | ---: | ---: | ---: |
| FilmGrain: new CPU and WGSL algorithm, amount and seed | 8 | 5 | 8 | 5 |
| SoftGlow: composition using existing kernels | 2 | 1 | 8 | 4 |
| TransformDrift: local-time transform hook | 2 | 1 | 8 | 4 |

FilmGrain's eight maintained files are the catalog, local semantic module,
logical-operation enum, common kernel declaration, CPU algorithm/dispatch,
WGPU registry, WGPU record/packing, and WGSL source. Its five registration sites
are the catalog row, logical-operation variant, common kernel row, CPU row and
WGPU row. A separately organized CPU module adds its ordinary module declaration;
the trial keeps its small algorithm in the existing CPU effects file.

SoftGlow and TransformDrift edit only the catalog and their local module.
SoftGlow emits four passes using three existing kernels and retains the original.
TransformDrift applies its own transform with zero kernels/passes and no central
evaluator edit. It preserves the existing Image/Shape/Text/Group source policy.

Each trial changes eight generated files out of 23 managed artifacts: high-level
class/stub, advanced handles, three export files, schema and parameter reference.
The earlier generator manages 19 files. Neither count means all generated files
change for every effect. The ordinary trial has five test-only paths; composite/transform each have
four. These include the new rendering test, exporter/catalog-count expectations,
whole-catalog conformance cases and, for a new kernel, shader/packed-record
fixtures. Common kernel.rs also has ordinary-trial test edits but is already
counted as maintained. Existing frozen rows are retained. Distinct changed paths
are 21 ordinary and 14 composite/transform; the measured implementation footprint
excludes these necessary test edits explicitly.

The historical audit's approximately 27-file estimate is not a recovered
measurement: `REPORT(6).md` was unavailable. The following before counts come
from independently counting source integration points at `8fb4847`; they are
hypothetical additions, not executed old-architecture implementations. After
counts are actual patches and commands. The matched ordinary route uses scalar
amount plus integer seed and an inline CPU algorithm, like the FilmGrain trial.

| Scenario | Before maintained files / sites | After maintained files / sites | Generated files changed before / after |
| --- | ---: | ---: | ---: |
| Ordinary, scalar-only, new CPU/WGSL kernel | 26 / 33 | 8 / 5 | 5 / 8 |
| Composite, reusing existing operations | 18 / 20 | 2 / 1 | 5 / 8 |
| Transform, scalar-only, own algorithm module | 21 / 22 | 2 / 1 | 5 / 8 |

A Vignette-shaped effect also has plain-track fields: its inline-CPU before
route is 26 files / 36 sites. A dedicated CPU module adds two files / one site
to either ordinary before route (28 / 34 scalar-only; 28 / 37 Vignette-shaped).
These differences explain the audit's approximate file estimate without
presenting unlike scenarios as exact equivalents.

Before ordinary maintained files: 13 core files, common kernel file, CPU effects
file, five WGPU files, and six Python/metadata files. Scalar-only core has 13
registration sites (catalog, authored kind, property enum/name, authored enum/id,
compiled enum/kind/scalar visitor, evaluated enum, normalization/remapping,
logical operation). Common has four, CPU one, WGPU seven, Python eight.
Plain-track enum/name/visitor add three sites for the Vignette-shaped route.
Constraint implementation bodies and typed Python class implementation are
semantic work, consistently excluded from registration-site counts.

Before generated changes are high-level module/stub, advanced collection
helpers, schema and parameter reference. The Python module and advanced file
also contain handwritten edits in that architecture, so maintained/generated
path sets overlap; do not sum them as distinct files. After these mechanical
sections and exports are generated. Optional Python ergonomics remain explicit.

Before command routes are inferred from the existing recipes, not measured by
running a new baseline effect: `just gen-effects`, `just gen-effects-check`,
focused crate tests, native rebuild/Python tests and strict adapters. After
trials actually run `just gen-effects`, full `just effect-check` (including
`gen-effects-check`), strict hardware facade rendering and focused Clippy;
A additionally rebuilds/imports the native package and checks generated exports,
signatures, helpers, canonical lowering, validation, CPU pixels and alpha. Each
full trial recipe reports six exporter/schema tests, 19 generator tests, core
349 plus one integration, common 64, CPU 175 and focused WGPU 122 passing. All
three final patches/measurements are based on `cfe04bc` and retain the original
`de5f036` trial artifacts separately. Scratch is restored clean at `cfe04bc`,
including removal of temporary native libraries from its Python source tree.
`just effect-check` now combines generation and core/common/CPU/focused
WGPU tests; it does not replace native or strict adapter verification.

Before exhaustive operation matches caught omitted operation cases, but common
inventory/bit lists, unconditional backend support, shader/parameter/upload
lists and Python exports were independent. After missing catalog modules or
required semantic functions fail compilation; operation/kernel/backend matches
are exhaustive, duplicate bits fail constant assertions, registered uniforms
receive structural ABI/upload checks, and generation freshness covers exports.
Actual mutations rejected duplicate scalar target 72, missing WGPU registration
and a missing transform hook. Property validation and numerical correctness still
need tests; the macros cannot prove an algorithm correct.

### Trial environment corrections

The first ordinary full recipe hit EGL BadAccess/device loss in two existing
readback callback tests under the host's default GLES path. The full pinned
Vulkan software recipe passed with those tests enabled. Consequently the guide
calls this a focused WGPU recipe, not an adapter-independent guarantee.

The worker's native environment mixed a CPython 3.12 venv with Nix's 3.13
sysconfig overrides. The build succeeded but import failed; selecting the
interpreter and clearing inherited Python sysconfig overrides produced the
correct 3.12 extension, and native smoke then passed. The smoke expectation
was also corrected to retain the existing normalized `keyframes: []` snapshot
shape. Original failure/retry logs are retained. Root's full package checks used
matching Python 3.13 and were unaffected. No production workaround was added.

## Compatibility audit

| Contract | Result and evidence |
| --- | --- |
| Rust public types/paths/enum construction and pattern shapes | Retained in original defining modules; workspace build/tests and frozen compiled/evaluated specimens pass |
| Canonical JSON | Frozen original serde/project values match; defaults, unknown fields, nulls and timing remain covered by existing suites |
| Python authoring API | Frozen imports, signatures, inheritance, slots, exports, lowering and validation timing pass within the 1,205-test native suite |
| Canonical schema | Byte-unchanged from baseline; generation and schema-validation/freshness checks pass |
| CPU/WGPU frames | All 240 original/final requests match their corresponding backend baseline byte-for-byte; actual reference/contact frames inspected |
| Shader labels/order/composed bytes | Frozen tests pass; WGSL source directory itself is byte-unchanged from baseline |
| GPU records and upload | Existing layouts/packed specimens preserved; all registered layouts receive named-offset/scalar-kind/WGSL/typed-upload checks |
| Backend selection/fallback/diagnostics | Existing facade and diagnostic suites pass; facade selection source is unchanged |
| Resource lifetime and pipeline behavior | Existing planning, submission/readback/cache/resource tests pass; executor, frame-plan and readback source unchanged; 28-created/27-reported counter discrepancy preserved |
| Ownership/dependencies | No new Cargo dependency or manifest changes; core/media graphs contain no renderer; feature-isolated compilation checked separately |

These are source, structural and specimen guarantees with actual rendered
coverage, not proofs of every parameter combination, resolution or adapter.
There is no intended compatibility exception or shipped new public effect.

## Local Git sequence

All work stays on `refactor/effect-architecture`, based on fetched `8fb4847`.
The implementation commits are:

```text
cebde57 docs: record effect architecture execution plan
f3e213a test(effects): freeze core compatibility specimens
39c7b5b refactor(core): derive effect models and property associations
8f167f0 test(effects): capture renderer registration compatibility
d583713 refactor(render): declare kernel inventory and operation mappings once
b780eeb refactor(wgpu): unify shader parameter and upload registration
62b77b7 refactor(cpu): bind capability decisions to typed effect dispatch
c8ab41d test(effects): enforce named uniform and exhaustive registry checks
01fd919 refactor(core): pilot effect-local semantic implementations
5da2d7f refactor(python): derive effect handles and exports
de5f036 refactor(core): derive traversal and dispatch to effect-local semantics
720f9eb refactor(core): derive canonical effect tags from catalog ids
cfe04bc refactor(core): derive semantic defaults and harden field traversal
```

The final guide/acceptance-record commit follows this sequence on the same
branch; its exact SHA is reported in the final handoff. No push, pull request,
merge or protected-branch modification was performed. There are two worktrees;
the scratch worktree is restored clean at `cfe04bc`.

## Completion / handoff

All eight milestones are complete. Implementation, extension exercises,
compatibility checks, documentation and local commits are finished; no temporary
effect ships. Final branch/HEAD and clean working-tree status are reported in
the handoff. No push, PR or merge is authorized or performed.

Remaining limits: typed point/interval field traversal recognizes the documented
bare/canonical paths, not arbitrary aliases; special algorithms, point temporal
policies, resource preparation and GPU packing remain explicit semantic work;
the bounded pass IR is unchanged and kernel bits remain limited to 64. The old
pipeline counter discrepancy remains. Performance neutrality, other platforms/
GPUs/APIs, and the 30 existing ignored manual suites are unverified.

Keep this completed acceptance record for the local feature branch. Lasting
architecture and contributor instructions are in the permanent guides; remove
the completed plan before a future authorized merge, following `PLANS.md`.


## Final hardening and integration (2026-10-28)

User scope: harden existing Phase 4 at `5433589`, preserve compatibility,
validate final changes and merge/push to `develop` without a PR. This supersedes
the earlier implementation-only authorization. Fetched remote: develop remains
`8fb4847`; main must remain untouched. Preserve untracked `vestra.zip` and the
existing scratch worktree. CodeGraph still exits 139; direct source inspection
is the navigation fallback.

### Remaining milestones

- [ ] Compile-time authored/compiled field classification and complete, unique
  scalar/plain associations; readable negative compilation fixtures, including
  unknown types, qualified types, serde attributes and palette exceptions.
- [ ] Adapter-independent effect-check boundary; strict suites retain device,
  pipeline, submission/readback and rendering assertions (adapter worker owns
  WGPU test gates, recipes/scripts and testing guide).
- [ ] Final Rust, explicit Python native rebuild/API/schema, package wheel/sdist,
  feature isolation, CPU/software/hardware rendering validation.
- [ ] Controlled baseline/candidate performance samples (read-only performance
  worker); independent final correctness review and fix confirmed findings.
- [ ] Update extension guide; transfer enduring knowledge, remove this completed
  plan per PLANS.md; commit, reconcile remote develop, merge and confirm push.

### Investigation

Authored/compiled/evaluated enums and target identities are generated. Existing
patterns and typed visitors reject nonexistent/wrong-kind associations, while
Rust enum definitions reject duplicate names/discriminants (including reserved
RotationDegrees=72). A declared scalar/plain field omitted from associations can
compile because visitors use `..`. Existing whole-catalog tests validate declared
associations/descriptors but do not enumerate all typed fields independently.
Palette input preserve_authored_track markers intentionally skip normalization
and transition remapping while remaining in dependency/property visitors.
Point/timing fields use separate projections; custom temporal/resource hooks
remain explicit. Hardening will add checks within the existing macro grammar,
without changing enum shapes, semantics, targets or renderer algorithms.

### Evidence and handoff

Pending final checks; old results above are historical baseline evidence only.
