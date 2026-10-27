# Renderer ownership cleanup and extraction review

Status: in progress — Phase 2 complete; awaiting explicit Phase 3 design approval.
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

- [x] Audit current ownership, callers, APIs, features, tests and Phase 1 integration.
- [x] Establish current baseline: full Rust/Python/generation/schema, four
  renderer configurations, software/hardware WGPU, representative frames,
  canonical CPU benchmark and available memory/dependency/build measurements.
- [x] Make minimal blocker cleanup in focused tested commits; no extraction.
- [x] Repeat required validation and inspect actual rendered output; investigate
  failures without weakening tests, tolerances or golden references.
- [x] Deliver ownership matrix, dependency diagrams, exact APIs/layout/features,
  test/CI migration, risks, commit-sized extraction plan and measured evidence.
- [x] Stop at Phase 2 review and request approval before Phase 3.

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
read-only audits completed; integrated design draft is
[render-extraction-review.md](render-extraction-review.md). Raw investigator
notes are `/tmp/vestra-{shared,wgpu,build}-audit.md`.

Baseline source remains unchanged from upstream; only planning documents exist.
Evidence is under `target/render-boundaries/baseline/`:

- `nix develop -c just check` passed (generation/exporter, formatting,
  workspace check/Clippy/tests, schema and docs). Rust totals include 6 exporter
  tests: 1,138 passed, 26 ignored, zero failures. The first attempt was interrupted
  after ten concurrent linkers caused paging; retained its log. Subsequent native
  commands use `CARGO_BUILD_JOBS=2`, with no cache deletion or profile changes.
- Four isolated renderer feature selections passed: neither 54/0 ignored,
  CPU 238/9 ignored, WGPU 192/3 ignored, both 442/20 ignored. Plain-shell runtime
  GPU tests can skip unavailable adapters; strict validation is separate.
- `cargo tree` normal/build unique packages: neither/CPU 142; WGPU/both 182.
  Neither/CPU activate no WGPU/Naga package. Full feature trees retained.
- Generation freshness/exporter/generator tests passed independently.
- Plain default-shell Python baseline: 1,166 passed, 34 skipped, five WGPU
  high-level text cases failed with no compatible adapter; this shell lacks a
  Vulkan loader in its LD_LIBRARY_PATH. Retrying with explicit adapter runtime
  remains required, not a code fix.
- Hardware discovery with the loader supplied by `.#wgpu-software`, its forced
  fallback unset, and NVIDIA ICD explicitly selected reports Vulkan, GTX 1650
  SUPER, discrete_gpu, driver NVIDIA 595.71.05. Exact invocation:
  `nix develop .#wgpu-software -c env -u VESTRA_WGPU_FORCE_FALLBACK VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json target/debug/examples/wgpu_adapters`.
- Software validation passed with llvmpipe LLVM 21.1.8 / Mesa 26.1.5.
  First baseline command batch and all subsequent validation sessions are terminal;
  exact command/results are retained in JSON and logs.
- Baseline manifest hashes all 36 WGSL files and both embedded font/license files.
  A local frame capture helper lives under ignored `target/render-boundaries/`.

Baseline completed: strict software and NVIDIA Vulkan wrappers passed, including
the hardware CLI regression; full Python passed 1,205 tests on each adapter.
Captured canonical, geometric-mask and ASCII sequences (18 unique frames per
backend, forward/backward/repeated) and inspected contact sheets. Independent
media and SDK no-backend checks passed; the renderer emits 45 existing unused/
dead-code warnings in the no-backend consumer build. Smoke and both ten-workload
canonical suites passed serially. Raw suites are
`target/benchmark-results/render-boundaries-{cpu,hardware}-baseline-20261027/`.
CPU used eight workers; GPU used three in-flight slots. Existing memory sampler
with psutil 7.2.2, 20 ms process-tree RSS sampling measured 777,908,224 bytes for
one unwarmed production-edit benchmark including fixture generation. This is a
separate instrumented memory sample, not a canonical timing sample.

The first CPU smoke inherited the software shell's strict WGPU requirement and
stopped before publishing a suite. Correct CPU measurements unset both strict
requirement flags; the failed log remains. No source/algorithm change was made
to obtain benchmark results. Baseline command sessions are now terminal.

Source cleanup:

- `88fa011`: move shared shape preparation to `src/shape_raster.rs`, retaining
  the logical module, visibility and all pixel code. Focused shared/no-backend
  shape tests 4 passed; WGPU-only shape tests 4 passed; CPU immutable-pixel
  sharing regression 1 passed. Diff contains only file relocation, module
  documentation and removal of the special path attribute.
- `263c019`: WGPU shader methods converted to backend-local free functions.
  Transformation asserts preserved every label/include literal and match-arm
  count. Pipeline/conformance tests 2 passed; combined-WGSL Naga test 1 passed.
  Kernel IDs, capability tables and shader assets remain untouched.
- `d27ecb4`: six WGPU resource tests are independent of CPU. All six executed
  and passed on strict NVIDIA Vulkan; padded-row CPU/WGPU comparison remains
  CPU-gated and separately passes. No assertions/tolerances were changed.

Full final verification passed against code HEAD
`d27ecb48a94072a69889ab3a8d8246f483e95981`: contributor gate (1,138 Rust including
six exporter tests, 26 ignored; 12 generator tests; Clippy/format/schema/docs),
four renderer modes (54/238/198/442 passes, 0/9/3/20 ignored), and independent
media/SDK no-backend checks. WGPU-only gained exactly the six resource tests.
Native package was forced to rebuild through
`uv sync --locked --extra dev --reinstall-package vestra`; all 1,205 Python tests
pass on each actual software/NVIDIA adapter, with zero skips, plus compileall.
Strict software and NVIDIA wrappers pass (92 strict tests, 11 ignored; NVIDIA
CLI encoded regression 1 passed). Software emits seven hardware-conformance skip
markers; NVIDIA emits none. All final sessions 51677/45043 are terminal.

All 54 before/after raw RGBA frames match directly (12,441,600 bytes), including
stable reverse/repeated requests. Official mask/ASCII comparisons pass at zero
tolerance, and final contact/difference images were inspected. All 38 embedded
asset hashes, the complete shape pixel body and four dependency trees are
unchanged. Detailed results and deliberate limitations are in the review report.

The first final `just check` invocation inherited global VESTRA_REQUIRE_WGPU=1
with the NVIDIA-only ICD. Two CLI backend-selection tests deliberately set
FORCE_FALLBACK=1 and cannot obtain a software adapter in that environment;
their global strict branches then fail. Retained the failed log and JSON.
Re-running general tests with both strict requirement flags unset, as the
existing verify-wgpu wrapper does, passes the focused three-test CLI suite.
Strict requirements remain scoped to actual GPU tests/Python and wrapper checks;
no source/test assertion was changed. This is an invocation correction.

Fresh review found no blocking source or design issue. Two additional Phase 3
cfg(test) cross-crate callers were verified and documented with existing-API
replacements; no production test hooks were added. Runtime/frame verification
remains the coordinator's responsibility.

## Decisions and discoveries

2026-10-27: Explicit review gate belongs after Phase 2 cleanup and validation;
the original assignment does not authorize Phase 3 extraction.

2026-10-27: Only demonstrated Phase 2 source changes selected: WGPU-owned free
kernel source/label functions; shared shape file relocation; six resource tests
available without CPU. Defer CPU timings/counts/surface blending as one coherent
Phase 3 move, along with prepared-resource getters and cross-crate visibility.
Existing pixel/reference tests cover these mechanical changes; do not add
implementation-mirroring tests or change rendering algorithms to manufacture
a failing behavior test.

## Completion / handoff

Phase 2 cleanup and audit/design are complete on `refactor/render-boundaries`.
See [the complete checkpoint report](render-extraction-review.md), including
baseline/final commands, adapters, benchmarks, memory scope, errors corrected
through invocation only, ownership APIs, CI/packaging and staged extraction plan.
Fresh review found no blocking issue. No crate was extracted, no API/algorithm/
shader/cache/scheduler/generator contract changed, and no push/merge/PR occurred.
Local main/develop still match the initial SHAs. Final handoff records the
documentation commit and clean-tree check.

Wait for explicit user design approval before creating `refactor/render-crates`.
If approved, stack it on the final Phase 2 branch commit unless these changes
have already merged into develop; incorporate amendments into this design.
Phase 3 includes fresh independent-package isolation, distributions, complete
runtime/public interface and controlled performance/memory comparisons. Optional
stress/4K/exhaustive checks and other graphics APIs/platforms remain unverified.
Do not implement Phase 4; recommendations follow actual Phase 3 ownership.

Preserve this plan and review report through the approval checkpoint. Before
eventual merge, transfer lasting facts to permanent docs and remove completed
execution material according to [PLANS.md](../../../PLANS.md).
