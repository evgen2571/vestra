# Documentation rewrite inventory

Status: non-normative planning record. This is a snapshot for the Vestra
documentation rewrite, not current user or reference documentation. It records
what the repository contains and where later documentation work should move it.

Snapshot date: 2026-08-20
Snapshot commit: `8d5964d` (`Migrate diagnostic codes to Vestra namespace`)

## Repository snapshot

The workspace has six crates:

| Crate | Current responsibility | Evidence to use later |
| --- | --- | --- |
| `vestra-core` | Project model, semantic validation, timeline, plan compilation/evaluation, signals, effect and audio descriptors | `crates/vestra-core/src/project/`, `src/validation/`, `src/plan/`, `src/plan_audio.rs`, unit tests |
| `vestra-render` | Renderer abstraction, CPU renderer, WGPU renderer, geometry, text, effects, backend capability checks | `crates/vestra-render/src/`, renderer tests, adapter example |
| `vestra-media` | FFmpeg/FFprobe probing, video/audio decode, audio analysis, sinks, encoding and publication helpers | `crates/vestra-media/src/`, media tests |
| `vestra` | Public Rust SDK, loading, validation/preflight, preparation, frame/video orchestration, events, results and errors | `crates/vestra/src/`, `crates/vestra/tests/` |
| `vestra-python` | PyO3 native bindings for the SDK | `crates/vestra-python/src/`, `python/vestra/_native.pyi` |
| `vestra-cli` | `ve` Clap surface, logging, terminal/JSON output, progress, reports and exit behavior | `crates/vestra-cli/src/`, `crates/vestra-cli/tests/` |

The pre-existing repository has 8,691 lines in `docs/**/*.md`; `README.md` is
660 lines. A previous validation snapshot estimated the broader documentation
material at about 9,566 lines because it included audit/data material. The
important fact is the shape of the tree: current-intent pages and phase/audit
records share the same top-level area, and there is no reader-oriented
`docs/index.md`.

The existing documentation groups are:

```text
README.md
docs/architecture.md
docs/architecture/
docs/audits/
docs/benchmarks/
docs/specs/
feature pages: particles, shapes, text, video-media, video-source
Python pages: editing API, parity audit, migration, source extension guide
WGPU page: wgpu-renderer.md
```

`examples/`, `schemas/`, `scripts/`, `.github/workflows/ci.yml`, Python tests,
and Rust tests are implementation evidence. They are not all documentation,
but later reference pages must link their claims to them.

## Source-of-truth hierarchy

For current behavior, use this order, with one qualification: a source is
authoritative only for the boundary it owns.

1. Current implementation and public exports.
2. Current tests, especially contract and serialization tests.
3. Checked-in schema, Python stubs, and other generated interfaces.
4. Current CLI definitions and CLI behavior tests.
5. Runnable examples that still execute against the current API.
6. Current-intent documentation, after it agrees with the sources above.
7. Historical audits, migrations, benchmarks, and phase reports.

The checked-in `schemas/project.schema.json` is a hybrid artifact. Its template
provides the base definitions, while `crates/vestra-cli/src/cli/commands/schema.rs`
injects effect descriptors, audio effect descriptors, and the Spectrum2D Nyquist
bound. The schema contract in `.github/workflows/ci.yml` and `scripts/check.sh`
regenerates it and compares the result. Future project-format documentation
must therefore cite the model, semantic validation, schema template, generator,
and schema tests together.

Historical documents can prove what a phase claimed or what was tested then.
They cannot override current behavior. A historical code mention is evidence
for a migration audit, not a current diagnostic or API contract.

## Current public and authoring hierarchy

The surfaces that future documentation must keep distinct are:

| Layer | Verified current surface | Future documentation |
| --- | --- | --- |
| High-level Python editing | `vestra.Project` owns a root `Composition`; a composition owns `Layer` placements; `CompositionLayer` owns a child composition; `Source` values come from `vestra.sources` | Getting-started and Python guides, then `reference/python-api.md` |
| Advanced/canonical Python authoring | `vestra.authoring.ProjectBuilder` and owned assets, clips, tracks, effects, transitions, flashes, presets, and audio timeline | Advanced authoring guide and Python reference |
| Immutable/runtime Python SDK | `ProjectSnapshot`, `Editor`, `PrepareOptions`, `PreparedProject`, `RenderRequest`, `RenderEvent`, and `RenderResult`, exposed through public Python wrappers and `_native.pyi` | Rendering/preparation guide and Python reference |
| Rust SDK | `vestra::Project`, `Editor`, `PreparedProject`, `PrepareOptions`, `RenderRequest`, `RenderEvent`, `RenderResult`, diagnostics, cancellation, backend preference and adapter DTOs | Rust SDK reference and lifecycle concept page |
| CLI | Binary `ve`; commands `validate`, `inspect`, `render`, `generate-schema`, and `version`; global repeated verbosity; human/JSON results and progress options | CLI getting-started, guides, and CLI reference |
| Canonical JSON | `vestra-core` project model, schema version `3`, serializer, semantic validation, schema file and schema generator | Project-format reference |

`vestra._native` is an implementation-level binding. It is not the normal
Python authoring entry point. `ProjectBuilder` remains supported and canonical
for exact JSON control; it is not merely obsolete migration code.

## Current feature inventory

This is a source map, not the final support matrix.

| Category | Current evidence to derive later |
| --- | --- |
| Sources | `crates/vestra-core/src/project/model/` visual source definitions; `python/vestra/sources/`; `schemas/project.schema.json`; source-specific Python tests. Current canonical variants include Image, Video, SolidColor, Shape, Text, Spectrum2D, ParticleSystem, and Group. High-level nested composition is presented as `CompositionLayer`, not as an ordinary user-facing `Source`. |
| Effects | Core effect descriptors and validation in `crates/vestra-core/src/effect_definition.rs` and `src/validation/effects.rs`; Python effects modules; Rust/Python effect tests; schema generator. |
| Transitions | Core transition definitions/validation/compiler; `python/vestra/transitions.py`; `test_authoring_transitions_flashes.py`, `test_high_level_transitions_f2.py`, and nested transition tests. |
| Flashes and presets | Core flash/preset model and compiler; `python/vestra/flashes.py`, `presets.py`; authoring frame and timeline tests; `examples/python/v2/05_transition_flash_preset_post_effect.py`. |
| Animation | Core tracks, keyframes, interpolation and plan evaluation; Python property/track modules; animation and frame tests. Time-domain claims must cite these tests. |
| Audio | Core audio model and validation; `src/plan_audio.rs`; `vestra-media/src/audio_graph.rs`; audio tests and Python audio tests. |
| Signals/audio reactivity | Core signal validation/compiler and audio analysis in `vestra-media/src/audio_analysis/`; `python/vestra/signals.py`; `examples/python/08_audio_reactive.py`, `v2/03_audio_reactive_signal.py`, and signal tests. |
| Nested compositions | High-level `CompositionLayer` in `python/vestra/editor.py`; lowering in `python/vestra/lowering.py`; nested group plan tests, high-level nested tests, and transition tests. |
| CPU rendering | `vestra-render/src/cpu/`, backend capability checks, renderer tests, and the public render regression suite. |
| WGPU rendering | `vestra-render/src/wgpu/`, backend selection and adapter discovery, WGPU frame tests, and `scripts/verify-wgpu*.sh`. |
| FFmpeg/media | `vestra-media/src/ffmpeg.rs`, `probe.rs`, `video.rs`, `audio_graph.rs`, `sink.rs`, `output.rs`, plus media and render tests. |

Later work should turn these sources into a versioned support matrix. This
phase deliberately does not make one.

## Verified stale or contradictory documentation

| Existing location | Current claim | Current evidence | Disposition |
| --- | --- | --- | --- |
| `README.md`, source links near the opening feature list | The shape guide is `docs/architecture.md`. | `docs/architecture.md` is an architecture page; `docs/shapes.md` is the dedicated shape page. | Rewrite the link and move the source overview to a source reference/guide. |
| `README.md`, Group limitations in Advanced Python authoring | Internal Group transitions are unsupported. | `crates/vestra-core/src/plan/tests.rs` has `nested_generic_transitions_compile_locally_and_count_all_associations`; high-level composition owns nested transitions through `CompositionLayer`. | Rewrite the capability statement after a full support review. Do not infer that every historical Group limitation is still true. |
| `README.md`, the old audio paragraph around the advanced API | It says schema version 2 is current. | Current model, schema, Python fixtures, loader, and tests use schema version 3. | Remove or rewrite as historical only. |
| `README.md`, video status in the Python package-development material | Video assets remain future work. | Current Python `Video` source, `test_high_level_video.py`, `test_render_video.py`, core video slots, media decode, and `examples/python/09_video_source.py`. | Delete the stale statement and document current video behavior in the video guide/reference. |
| `README.md`, sections labeled Phase 7A/7B/7C | Normal-user API description is organized by implementation phases. | The public wrappers and `.pyi` expose one current SDK lifecycle, and the phase reports are already in `docs/audits/`. | Move phase history to `docs/history/audits/`; rewrite README as user/API material. |
| `docs/architecture.md`, optimization and cache sections | Phase 10B/10C are embedded in current architecture. | Headings and prose explicitly name those phases; current crate ownership is confirmed by manifests and source. | Split current architecture from historical optimization reports. |
| `docs/architecture.md`, audio heading | `Audio timeline (schema v2)`. | Current model and schema are version 3; `docs/specs/project-format.md` already says version 3. | Rewrite the current page and preserve the old phase context only in history if needed. |
| `docs/wgpu-renderer.md`, verification/benchmark material | Architecture, correctness validation, troubleshooting, and benchmark conclusions share one page. | `scripts/verify-wgpu.sh` explicitly labels software correctness; `verify-wgpu-hardware.sh` requires a classified discrete/integrated adapter; CI runs software WGPU separately. | Split into architecture, GPU validation, backend reference, troubleshooting, and historical benchmark material. |
| `crates/vestra/src/project/validated.rs` diagnostic message | Unsupported projects are told the supported version is 2. | The same loader rejects values other than 3; schema and tests use 3. | Product/source defect to fix separately. Record now; do not change in this documentation phase. |

The last item is code, not documentation, but it is a direct source/documentation
inconsistency and must be resolved before publishing a project-format reference.

## Historical material classification

The existing historical collections should move under `docs/history/` without
rewriting their conclusions.

### Audits

| Existing files | Classification | Target |
| --- | --- | --- |
| `docs/audits/effects-ready-v1.tsv`, `phase3.tsv`, `phase4.tsv`, `phase-f.tsv` | `GENERATED_OR_DATA` | `history/audits/` or `history/benchmarks/` after identifying their producer |
| `docs/audits/phase5-finalization.md`, `phase6a.md`, `phase6b.md`, `phase6c.md`, `phase7a.md`, `phase7a0.md`, `phase7b.md`, `phase7c.md`, `phase8.md`, `phase8-authoring-conformance.md`, `phase8a.md`, `phase8b.md`, `phase8c-a.md`, `phase8c-b.md`, `phase8c-c.md`, `phase8c-d.md`, `phase8c-e.md`, `phase9a.md`, `phase9b.md`, `phase9c.md`, `phase9d.md`, `phase10a.md`, `phase10b.md`, `phase10c.md`, `phase10d.md` | `ARCHIVE_MIGRATION` | `history/audits/` |
| `docs/audits/cpu-alpha-composition-2026-08-13.md`, `cpu-chromatic-aberration-2026-08-13.md`, `cpu-color-adjust-lut-2026-08-12.md`, `cpu-gaussian-bloom-algorithm-2026-08-13.md`, `cpu-gaussian-kernel-2026-08-12.md`, `cpu-rasterization-2026-08-12.md`, `cpu-sharpen-isolation-color-adjust-2026-08-12.md`, `cpu-source-raster-stage2-2026-08-13.md`, `cpu-zoom-blur-2026-08-12.md`, `cpu-zoom-blur-stage2-2026-08-13.md`, `multicore-cpu-renderer.md`, `static-visual-fast-path.md` | `ARCHIVE_BENCHMARK` | `history/benchmarks/` or `history/audits/`, retaining the distinction between measured results and implementation review |
| `docs/audits/performance-baseline-2026-08-12.md`, `performance-phase-final-2026-08-13.md` | `ARCHIVE_BENCHMARK` | `history/benchmarks/` |
| `docs/audits/procedural-signal-audio-modulation.md`, `spectrum2d-foundation.md`, `spectrum2d-layouts-styles-v1.md`, `visual-sources-v1e-performance.md` | `ARCHIVE_AUDIT` | `history/audits/`, with any raw benchmark data kept beside the report |

The 58-file audit collection is historical by role even when a paragraph still
contains a useful current fact. Normative facts should be extracted into new
current pages and then linked back to the historical evidence.

### Other historical/current-intent files

| Existing file | Classification | Future action |
| --- | --- | --- |
| `docs/architecture.md` | `SPLIT` | Replace with current development architecture pages; archive phase-specific material. |
| `docs/architecture/crate-refactor.md` | `ARCHIVE_MIGRATION` | Move to `history/migrations/`; its title and content are explicitly Phase 4. |
| `docs/benchmarks/cpu-renderer-baseline.md`, `docs/benchmarks/effects-ready-v1.md` | `ARCHIVE_BENCHMARK` | Move to `history/benchmarks/`; keep raw result context. |
| `docs/audits/phase10d-effect-results.json`, `phase10d-layer-results.json`, `phase10d-layer25-results.json`, `phase10d-layer50-results.json`, `phase10d-preparation-results.json`, `phase10d-random-results.json`, `phase10d-repeated-results.json`, `phase10d-results.json`, `procedural-signal-audio-modulation-benchmarks.json`, `spectrum2d-foundation-benchmarks.json`, `spectrum2d-layouts-styles-v1-benchmarks.json` | `GENERATED_OR_DATA` | Move with the audit/benchmark that produced each file. Do not rewrite or delete in this phase. |
| `docs/python-editing-api.md` | `CURRENT_REWRITE` | Split into a short guide and `reference/python-api.md`; preserve the verified current examples. |
| `docs/python-editing-api-parity.md` | `ARCHIVE_AUDIT` | Extract current API facts into reference pages, then archive the parity record. |
| `docs/python-editing-api-migration.md` | `ARCHIVE_MIGRATION` | Keep as migration history, with links to the current Python guide. |
| `docs/python-source-extension-guide.md` | `CURRENT_REWRITE` | Move to `development/extending/source.md`. |
| `docs/specs/project-format.md` | `CURRENT_REWRITE` | Move/rewrite as `reference/project-format.md`; use model, schema, generator, validation, and tests together. |
| `docs/shapes.md`, `docs/text.md`, `docs/particles.md`, `docs/video-media.md`, `docs/video-source.md` | `CURRENT_REWRITE` | Split between concept, guide, and source reference pages. |
| `docs/wgpu-renderer.md` | `SPLIT` | Map its sections to the WGPU architecture, GPU validation, backend reference, and troubleshooting pages below. |
| `AGENTS.md` | `CURRENT_KEEP` | Repository instructions, not reader documentation. Keep at the repository root. |

No file is marked for immediate removal. A later migration pass may mark old
duplicates `REMOVE_AFTER_MIGRATION` only after links and current replacements
are verified.

## Diagnostic namespace audit

Before this inventory was added, the working tree contained no `MVP-` strings
in `README.md` or `docs/`. That pre-inventory snapshot contained `VESTRA-`
strings in 13 documentation files, and current source and tests use the
`VESTRA-*` namespace. This audit mentions both names deliberately and should
be excluded from namespace counts.

Git history provides direct evidence of a mechanical migration. Commit
`8d5964d` changed `MVP-*` to `VESTRA-*` in thirteen audit/spec/WGPU files, including
`phase6a.md`, `phase6c.md`, `phase7a0.md`, `phase7c.md`, `phase8b.md`,
`phase8c-a.md`, `phase8c-d.md`, `phase8c-e.md`, `phase9a.md`, `phase9b.md`,
`phase9c.md`, `docs/specs/project-format.md`, and `docs/wgpu-renderer.md`.
The commit changed the same identifiers across source and tests. This is
strong evidence for the affected historical values, so future history cleanup
may restore original `MVP-*` values in those records if preserving original
reports is required. The restoration must be selective and based on that
commit, not a guessed mass replacement. No restoration is made here.

## README section inventory

`README.md` should become an entry point, not a 660-line manual.

| Current section/range | Classification | Target |
| --- | --- | --- |
| Opening description and capability list | `KEEP_CONCEPT` | Short README overview, with corrected links. |
| `## Python editing API` and its example | `MOVE_GETTING_STARTED` | `getting-started/python-quickstart.md` plus a small README example. |
| `## Advanced Python authoring` | `MOVE_GUIDE` | Python authoring/model guide and `reference/python-api.md`. |
| Advanced track/effect/transition/audio/signal details | `MOVE_REFERENCE` | Python guides and exact reference pages. |
| Build, CLI, CI, environment, and verification material | `MOVE_DEVELOPMENT` for maintainer commands; `MOVE_GUIDE` for CLI use | Installation, CLI guides, development/testing, WGPU validation, and environment reference. |
| `## Rust SDK`, project lifecycle, prepared frames | `MOVE_REFERENCE` | `reference/rust-sdk.md` and rendering lifecycle concept. |
| API stability and package development | `MOVE_DEVELOPMENT` | Contributing/testing and Python development pages. |
| Phase 7A/7B/7C narrative and old video-future claim | `DELETE_STALE` and `MOVE_HISTORY` | Remove from README; retain phase evidence under history. |

The future README should contain only: what Vestra is, major capabilities,
maturity/status, an installation entry point, one tiny Python example, one tiny
CLI example, and links onward.

## Existing to target mapping

| Existing | Classification | Future destination(s) | Action |
| --- | --- | --- | --- |
| `README.md` | `CURRENT_REWRITE` | `README.md`, guides and references | Shrink to entry point; remove stale and phase prose. |
| `docs/architecture.md` | `SPLIT` | `development/architecture/*`, `concepts/rendering-lifecycle.md` | Split current ownership/control flow from optimization history. |
| `docs/architecture/crate-refactor.md` | `ARCHIVE_MIGRATION` | `history/migrations/` | Move unchanged first. |
| `docs/python-editing-api.md` | `CURRENT_REWRITE` | `guides/python/*`, `reference/python-api.md` | Split task-oriented and exact contract material. |
| `docs/python-editing-api-parity.md` | `ARCHIVE_AUDIT` | `history/audits/` | Retain evidence; extract facts. |
| `docs/python-editing-api-migration.md` | `ARCHIVE_MIGRATION` | `history/migrations/` | Retain for users migrating old code, with current links. |
| `docs/python-source-extension-guide.md` | `CURRENT_REWRITE` | `development/extending/source.md` | Reframe as contributor/developer material. |
| `docs/specs/project-format.md` | `CURRENT_REWRITE` | `reference/project-format.md` | Rewrite from current truth map. |
| `docs/shapes.md`, `text.md`, `particles.md`, `video-media.md`, `video-source.md` | `CURRENT_REWRITE` | source references and Python guides | Split by reader task and exact contract. |
| `docs/wgpu-renderer.md` | `SPLIT` | WGPU architecture, GPU validation, backend reference, WGPU troubleshooting | Separate implementation, validation, contract, and failure help. |
| `docs/benchmarks/*` | `ARCHIVE_BENCHMARK` | `history/benchmarks/` | Preserve measured context, no new claims here. |
| `docs/audits/*.md` | `ARCHIVE_AUDIT`, `ARCHIVE_MIGRATION`, or `ARCHIVE_BENCHMARK` as listed above | `history/audits/`, `history/migrations/`, `history/benchmarks/` | Move by role, not filename alone. |
| `docs/audits/*.{json,tsv}` | `GENERATED_OR_DATA` | Beside its producing historical report | Preserve as raw evidence. |

### Path coverage ledger

This ledger makes the classification mechanically checkable. Every pre-existing
Markdown or audit-data path under `docs/`, plus the root instruction and README,
appears once here.

| Path | Classification |
| --- | --- |
| `AGENTS.md` | `CURRENT_KEEP` |
| `README.md` | `CURRENT_REWRITE` |
| `docs/architecture.md` | `SPLIT` |
| `docs/architecture/crate-refactor.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/cpu-alpha-composition-2026-08-13.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-chromatic-aberration-2026-08-13.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-color-adjust-lut-2026-08-12.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-gaussian-bloom-algorithm-2026-08-13.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-gaussian-kernel-2026-08-12.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-rasterization-2026-08-12.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-sharpen-isolation-color-adjust-2026-08-12.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-source-raster-stage2-2026-08-13.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-zoom-blur-2026-08-12.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/cpu-zoom-blur-stage2-2026-08-13.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/effects-ready-v1.tsv` | `GENERATED_OR_DATA` |
| `docs/audits/multicore-cpu-renderer.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/performance-baseline-2026-08-12.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/performance-phase-final-2026-08-13.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/phase-f.tsv` | `GENERATED_OR_DATA` |
| `docs/audits/phase10a.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase10b.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase10c.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase10d-effect-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d-layer-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d-layer25-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d-layer50-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d-preparation-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d-random-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d-repeated-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d-results.json` | `GENERATED_OR_DATA` |
| `docs/audits/phase10d.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase3.tsv` | `GENERATED_OR_DATA` |
| `docs/audits/phase4.tsv` | `GENERATED_OR_DATA` |
| `docs/audits/phase5-finalization.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase6a.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase6b.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase6c.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase7a.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase7a0.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase7b.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase7c.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8-authoring-conformance.md` | `ARCHIVE_AUDIT` |
| `docs/audits/phase8.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8a.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8b.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8c-a.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8c-b.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8c-c.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8c-d.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase8c-e.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase9a.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase9b.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase9c.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/phase9d.md` | `ARCHIVE_MIGRATION` |
| `docs/audits/procedural-signal-audio-modulation-benchmarks.json` | `GENERATED_OR_DATA` |
| `docs/audits/procedural-signal-audio-modulation.md` | `ARCHIVE_AUDIT` |
| `docs/audits/spectrum2d-foundation-benchmarks.json` | `GENERATED_OR_DATA` |
| `docs/audits/spectrum2d-foundation.md` | `ARCHIVE_AUDIT` |
| `docs/audits/spectrum2d-layouts-styles-v1-benchmarks.json` | `GENERATED_OR_DATA` |
| `docs/audits/spectrum2d-layouts-styles-v1.md` | `ARCHIVE_AUDIT` |
| `docs/audits/static-visual-fast-path.md` | `ARCHIVE_BENCHMARK` |
| `docs/audits/visual-sources-v1e-performance.md` | `ARCHIVE_BENCHMARK` |
| `docs/benchmarks/cpu-renderer-baseline.md` | `ARCHIVE_BENCHMARK` |
| `docs/benchmarks/effects-ready-v1.md` | `ARCHIVE_BENCHMARK` |
| `docs/particles.md` | `CURRENT_REWRITE` |
| `docs/python-editing-api-migration.md` | `ARCHIVE_MIGRATION` |
| `docs/python-editing-api-parity.md` | `ARCHIVE_AUDIT` |
| `docs/python-editing-api.md` | `CURRENT_REWRITE` |
| `docs/python-source-extension-guide.md` | `CURRENT_REWRITE` |
| `docs/shapes.md` | `CURRENT_REWRITE` |
| `docs/specs/project-format.md` | `CURRENT_REWRITE` |
| `docs/text.md` | `CURRENT_REWRITE` |
| `docs/video-media.md` | `CURRENT_REWRITE` |
| `docs/video-source.md` | `CURRENT_REWRITE` |
| `docs/wgpu-renderer.md` | `SPLIT` |

## Validated target documentation tree

The proposed tree fits the current code boundaries. No structural change from
the requested baseline is needed.

```text
README.md
docs/
├── index.md
├── getting-started/
│   ├── installation.md
│   ├── python-quickstart.md
│   └── cli-quickstart.md
├── concepts/
│   ├── authoring-model.md
│   ├── timeline-and-time.md
│   ├── rendering-lifecycle.md
│   ├── effects-transitions-and-animation.md
│   ├── audio-and-signals.md
│   └── rendering-backends.md
├── guides/
│   ├── python/
│   │   ├── projects-and-compositions.md
│   │   ├── sources-and-layers.md
│   │   ├── animation.md
│   │   ├── effects.md
│   │   ├── transitions-flashes-and-presets.md
│   │   ├── audio.md
│   │   ├── signals-and-audio-reactivity.md
│   │   ├── nested-compositions.md
│   │   └── rendering-and-preparation.md
│   └── cli/
│       ├── rendering.md
│       └── logging-and-progress.md
├── reference/
│   ├── project-format.md
│   ├── python-api.md
│   ├── rust-sdk.md
│   ├── cli.md
│   ├── diagnostics.md
│   ├── environment-variables.md
│   ├── feature-support.md
│   ├── sources/{image,video,solid-color,shape,text,spectrum2d,particle-system}.md
│   ├── nested-compositions.md
│   ├── effects.md
│   ├── transitions.md
│   ├── presets-and-flashes.md
│   ├── audio.md
│   └── backends.md
├── troubleshooting/{rendering,ffmpeg,wgpu,python}.md
├── development/
│   ├── architecture/{overview,crates,project-to-plan,render-pipeline,cpu-renderer,wgpu-renderer,media-io,python-bindings}.md
│   ├── extending/{source,effect,transition,backend}.md
│   ├── testing.md
│   ├── gpu-validation.md
│   ├── performance.md
│   └── contributing.md
└── history/
    ├── README.md
    ├── migrations/
    ├── benchmarks/
    └── audits/
```

`reference/sources/` intentionally omits Group. The canonical model has a
Group variant, but the high-level Python contract names the user-facing nested
composition placement `CompositionLayer`. Group internals belong in the
nested-compositions and project-format references until a separate public
Group contract is established.

## Canonical truth map for future reference pages

| Future page | Authoritative sources |
| --- | --- |
| `reference/cli.md` | Clap definitions in `crates/vestra-cli/src/cli/args.rs`, command implementations, CLI integration tests |
| `reference/project-format.md` | Core model/serialization, semantic validation, `schemas/project.schema.json`, schema generator, schema-validation and serialization tests |
| `reference/python-api.md` | `python/vestra` implementation and exports, `python/vestra/_native.pyi`, PyO3 exports, Python contract/typing tests |
| `reference/rust-sdk.md` | `crates/vestra/src/lib.rs` public exports, editor/prepared/application types, public SDK tests |
| `reference/feature-support.md` | Core source/effect/transition/audio descriptors, renderer dispatch/capability checks, media support, Rust/Python/CLI tests |
| `reference/backends.md` | `vestra-render` backend APIs and selection, SDK preparation/report DTOs, adapter discovery, backend tests |
| `reference/diagnostics.md` | Core `Diagnostic` and validation modules, SDK error exposure, CLI output/report code, Python diagnostics, tests |
| `reference/environment-variables.md` | CLI logging, renderer/backend selection, WGPU scripts, CI, and test fixtures |
| `reference/sources/{image,video,solid-color,shape,text,spectrum2d,particle-system}.md` | Corresponding core model/schema branches, Python source modules, renderer/media dispatch, source-specific tests and examples |
| `reference/nested-compositions.md` | Python `Composition`/`CompositionLayer`, lowering, core Group compiler/evaluator, nested composition tests |
| `reference/effects.md` | Effect descriptor catalog, validation, Python effects, schema generation, renderer kernels, effect tests |
| `reference/transitions.md` | Transition model/validation/compiler, Python transition API, transition tests |
| `reference/presets-and-flashes.md` | Core preset/flash definitions and compiler, Python modules, frame tests and examples |
| `reference/audio.md` | Core audio model/validation/plan, media audio graph and analysis, Python audio API, audio tests |
| `reference/backends.md` and `troubleshooting/wgpu.md` | Backend APIs plus `scripts/verify-wgpu.sh` and `scripts/verify-wgpu-hardware.sh`; keep software correctness separate from hardware claims |

Exact pages should name defaults, ranges, validation, support, and stability
only after checking all sources in their row. A test that covers one fixture is
not proof of a complete feature matrix.

## WGPU documentation inventory

The current WGPU material maps as follows:

| Existing material in `docs/wgpu-renderer.md` | Future destination |
| --- | --- |
| Backend ownership, command encoding, persistent textures/pipelines/readback, and CPU/WGPU boundaries | `development/architecture/wgpu-renderer.md` |
| Adapter discovery, strict hardware requirement, software correctness, WSL backend choice, and verification commands | `development/gpu-validation.md` |
| Requested backend versus actual backend, fallback metadata, adapter/device fields, and public selection contract | `reference/backends.md` |
| Missing adapter, device loss, backend failures, and common environment failures | `troubleshooting/wgpu.md` |
| Measured benchmark tables and deferred performance claims | `history/benchmarks/` |

The source scripts confirm that software WGPU correctness and hardware WGPU
validation are different classes. `verify-wgpu.sh` labels software correctness
and permits a selected software backend; `verify-wgpu-hardware.sh` requires a
classified discrete or integrated adapter. Future pages must not call Vulkan
llvmpipe hardware rendering or use it for hardware performance claims. On WSL,
the repository instructions identify Vulkan as llvmpipe and GL/GLES as the
possible D3D12/NVIDIA path when available. Discover adapters first, distinguish
requested from actual backend, and report hardware validation as blocked when a
real adapter cannot be obtained.

## Terminology

Use these terms consistently:

| Term | Meaning for future docs |
| --- | --- |
| Project | The authored graph or canonical project, depending on context; qualify with `high-level`, `canonical`, `snapshot`, or `prepared` when needed. |
| Composition | An ordered collection of layers local to one composition. |
| Layer | A timed placement of a source in a composition. |
| CompositionLayer | High-level Python layer placement that owns a child composition. |
| Source | A visual value such as Image, Video, SolidColor, Shape, Text, Spectrum2D, or ParticleSystem. |
| Nested composition | A composition placed by a `CompositionLayer`; the canonical lowering uses a Group representation. |
| Prepared project/render | The validated, probed, capability-checked state used for frame or video work. |
| CPU backend | The deterministic CPU renderer. |
| WGPU backend | The renderer using the WGPU graphics API; it does not imply a hardware adapter. |
| Requested backend / actual backend | The user's preference and the backend selected after preparation. |
| Graphics backend/API | The API exposed by an adapter, such as GL or Vulkan. |
| Adapter | The graphics device candidate discovered by WGPU. |
| Software adapter | A CPU-backed adapter such as llvmpipe. |
| Diagnostic | A structured error or warning with code, category, message, path, and related fields where present. |

Stop using phase labels as user-facing feature names, `MVP-*` as current
diagnostics, `schema v2` for the current project format, and vague claims that
video or nested compositions are future work. Preserve those phrases only in
historical context where the date and status are clear.

## Page-type standards

### Getting started

State the goal, prerequisites, steps, a minimal runnable example, expected
result, and links to the next task.

### Guide

State the task and when to use it. Show the workflow, important options,
pitfalls, and the related exact reference.

### Concept

Give the mental model, define core terms, show ownership or relationships, list
invariants, include a small example, and link to guides/reference.

### Reference

State the exact contract, types and values, defaults, validation rules,
supported features, and compatibility/stability notes where relevant. Cite the
implementation and tests that establish each non-obvious claim.

### Architecture

Describe responsibility, ownership, control/data flow, boundaries, invariants,
extension points, and failure/cancellation behavior where relevant. Keep phase
history and benchmark numbers in history.

### Historical

State the date or phase, mark the page non-normative, explain the original
context, and preserve the conclusion and evidence without presenting it as the
current contract.

## Next migration gates

Before rewriting current pages:

1. Fix or explicitly qualify the schema-version diagnostic inconsistency.
2. Build the current support matrix from descriptors, dispatch, and tests.
3. Decide whether to preserve original `MVP-*` values in archived reports using
   the migration commit as evidence.
4. Move historical files and raw data without changing their conclusions.
5. Create `docs/index.md`, then rewrite pages in the target tree and update all
   links from the shortened README.

This inventory intentionally does not perform those migrations, rewrite the
README, change APIs, alter schema semantics, restore diagnostics, or make new
GPU benchmark claims.
