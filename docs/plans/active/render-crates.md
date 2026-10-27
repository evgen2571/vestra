# Renderer crate extraction

Status: completed — implementation and required local validation passed
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

- [x] 1. Separate CPU timings, blend execution and host CRT; focused renderer tests.
- [x] 2. Extract common with narrow accessors, unconditional Gaussian storage and
  shared mapping/requirements; common and facade feature checks. Review boundary.
- [x] 3. Extract CPU with backend capability policy; CPU tests and isolation.
- [x] 4. Extract WGPU/shaders, split private/parity tests without losing assertions;
  WGPU/facade tests, asset hashes and strict software/hardware execution.
- [x] 5. Complete facade, media/SDK borrowed sink migration (distinct commit),
  test discovery and CI with meaningful backend isolation.
- [x] 6. Complete compatibility, packaging, benchmark/memory/build measurements
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
measurements. Fresh no-backend baseline passed: 54 tests.

Step 1: CPU timings, composition counters/surface execution and host CRT moved
under cpu. Reference surface test follows execution. Focused feature tests pass:
CPU 238/9 ignored; neither 52/0; WGPU 196/3. One CRT test and one surface test
now correctly require CPU (both previously ran in no-backend/WGPU builds).
Source-only relocation preserves reports and numerical bodies. No strict GPU
runtime claim is made by the plain-shell WGPU test run. Logs: step1-*.log.
The new media borrowed-slice tests reject the old CompletedFrame signature
(E0061), before implementation; media-borrowed-red.log records that check.

Media migration (early, distinct commit): FrameSink/FfmpegSink accept borrowed
RGBA plus frame number; all SDK/test/benchmark sinks and calls updated. Static
allocation/drop timing and copy metrics are preserved, and scoped writes borrow
the existing completed frame through join. Media tests: 112 passed/4 ignored;
SDK CPU engine 72 passed; application render 5 passed. New test overwrites and
reuses one caller buffer after two accepted writes; it checks ordering state,
not encoded pixels. Existing encoding/SDK/CLI tests cover output. Ordering
and invalid-size tests exercise the sink and retry without advancing ordering.
Normal/build/dev media tree saved to media-tree.txt; isolated build still pending.

Step 2: common now owns contracts/preparation, immutable Arc resources, geometry,
shared pixel/particle/numerical helpers, snapshots/cache and kernel mapping/sets.
Prepared fields stay crate-private with borrowed getters; font/text/shape builder
functions remain private. Geometry exposes needed scalar value records but keeps
logical preparation bounds and forward construction private. Kernel bitset mutation
is private. No new abstraction, frame copy, duplicate cache or ownership transfer.
Backend policy moved to CPU/WGPU support modules; selection still calls explicitly.
Gaussian calculations are unchanged, float weights retained in every build and
cache eviction tested inside common; CPU tests use existing shared preparation
and raster APIs instead of promoting cfg(test) hooks. Common: 63 passed/3 ignored;
facade lib CPU 175/6, WGPU 131/0, both 379/17, neither 0/0, all pass in the plain
shell (runtime adapter skips possible). CPU+common totals preserve 238/9 and
combined+common preserve 442/20. Follow-up WGPU-owned all-kernel support assertion
also restored; strict actual-adapter validation remains pending.

Common boundary review: dependencies contain only core and original preparation
libraries/serde/tracing; no execution, workers, GPU resources or media. The shared
cache type remains because both backends already use it, while instances remain
backend-owned. WGPU-only added retention is at most 16 * 65 * 8 = 8,320 coefficient
payload bytes per populated thread plus 16 Vec headers (384 bytes on this 64-bit
host), allocator overhead and cache-container effects; this is not a measured RSS
delta. CPU allocation/calculation and cache policy are unchanged.

Step 3: CPU package extracted with workers/effects/composition/profile/CRT and
BlueNoise include. Facade forwards its optional CPU feature and type/module
compatibility paths; validation is an explicit CPU-owned function. CPU package
175 passed/6 ignored, facade CPU/neither compile and pass. Saved normal/build/dev
tree excludes WGPU implementation/stack and Naga. Fresh isolated artifact proof
is part of final validation, not inferred from this warm package build.

Moved parity tests alongside this step because CPU's existing cfg(test)
render_frame helper cannot be called across a crate boundary. Facade uses a local
test adapter built only from submit/poll/flush; no production hook. Both-feature
facade unit and integration suites compile/pass; parity integration 70 passed/
11 ignored in plain shell. All 81 original facade parity tests remain; private
planning/resource assertions stay WGPU-owned. Source inventory and normalized
assertion audit: parity-test-migration.json and parity-assertion-audit.json
(stylization 144/144, crops 33/33, resources 66/66, parity 56/56 with one callback
syntax adaptation). Pure shared literals are tests/render_effect_cases.rs, used
independently by both owners, with no package edge. Strict runtime still pending.

Step 4: WGPU implementation/shaders extracted, private tests independent of CPU
including companion planning/resource cases. Backend package 142 passed/4 ignored.
All facade modes pass: neither 4, CPU 4, WGPU 2, both 2 unit + 70 integration/
11 ignored. Added focused unavailable-feature/CPU-selection contract tests so
no-backend retains direct policy evidence after common unit tests move away.
WGPU normal/build/dev graph excludes CPU. All 38 embedded assets byte-identical
to 69e8eb6 (asset-comparison.json). Workspace all-target/all-feature Clippy passes
with warnings denied. Removed unused test-only kernel bitset inspection rather
than expose it. Actual strict runtime/frame/resource validation follows next.
Historical report source-link targets follow relocated descendants; labels,
findings and measurements remain unchanged. Docs links pass (436, zero missing).

Step 5: strict wrapper explicitly collects WGPU-private and facade integration
gpu_ tests under the same strict/software/hardware/GL policy. Grouped CPU CI job
uses its own Rust cache and guards actual normal/build/dev trees; integration
checks GPU-only graphs before full-workspace unification. Full workspace/Python/
generation and docs-only/manual routing retained. Shell syntax/ShellCheck pass;
fake-Cargo checks four route/thread/flag cases, hardware software rejection,
list-only and 22 CI result-gate cases. Hosted CI cost remains unmeasured: the
extra CPU job duplicates some common/core/preparation cold work across caches.
Permanent architecture/extension/testing/media/GPU docs, generator destinations
and font notices updated. Generator output routing and syntax pass.

Initial fresh sdist inspection finds all four crates, all 36 WGSL files, common
font/license and CPU include table. Cargo's package file discovery omits the
shared repo-level Rust test cases; added exactly that file as an explicit sdist
include. Rebuild/archive extraction/wheel smoke remain in final acceptance.

Strict software wrapper passes on llvmpipe Vulkan (LLVM 21.1.8, Mesa 26.1.5): full
workspace plus 28 targeted WGPU-private tests/4 ignored and 70 facade parity/
11 ignored. Hardware conformance skips remain explicit on software. NVIDIA run
started using explicit ICD and Vulkan, with software fallback unset.
Final normal/build/dev graph inspection confirms media has no renderer,
cosmic-text, fontdb or tiny-skia package. Common/CPU graph has no WGPU/Naga;
WGPU graph has no CPU. Final fresh sdist inventory contains shared test source,
all required manifests and all 38 exact asset hashes; extracted source retained.
Independent focused read-only review found no blocking production issue; its
single evidence-text correction above is applied. See independent-review.md in
ignored evidence. Runtime/performance/isolated compiler/wheel gates remain open.

Final compatibility gates on source through 07d6096: NVIDIA strict wrapper passes
(GTX 1650 SUPER, Vulkan, driver 595.71.05), including targeted 28 WGPU-private/
4 ignored, 70 facade/11 ignored and encoded CLI comparison 1 passed. No hardware
runtime skip markers. Fresh native extension rebuild completed in 126.69 s;
Python has 1,205 passes/zero skips on each NVIDIA and software runtime, plus
compileall. `just check` passes format/generation/exporter/generator/workspace
check/Clippy/schema/docs: 1,149 Rust passes including 6 exporter tests, 30 ignored;
12 generator tests and no stale artifacts. These ignored occurrences include
four new split resource companions; original tests/assertions are retained.

All 54 raw RGBA frames match the final Phase 2 capture byte-for-byte (12,441,600
bytes), including repeated/backward requests. Official mask/ASCII comparisons
at zero tolerance report MAE/max error/mismatched pixels all zero. The current
CPU/software/NVIDIA contact sheet was visually inspected. This before/after
proof is separate from existing cross-backend tolerance/literal tests.
Media normal/build/dev unique packages: 173 before -> 65 after, with renderer
and preparation-font packages absent. Facade normal/build packages: neither
143, CPU 144, WGPU 184, both 185 (before 142/142/182/182).
Canonical CPU/NVIDIA performance and process memory capture are now running
serially with existing tooling; isolated compiler and installed packages next.

Final validation completed: the [18-point results report](render-crates-results.md)
records actual commands/results, layouts, graphs, compatibility, packaging,
performance/memory/build measurements and limitations. Runtime sources match both
measured candidate fingerprints; subsequent changes are documentation/skill only.
All 18 cold/warm/incremental compiler records were validated against real artifact
sets. Media cold/warm builds select no renderer/font preparation; CPU/WGPU builds
exclude their sibling. Source snapshots preserve the actual checkout and Git refs.

Matched canonical CPU/NVIDIA comparisons pass the unchanged comparer. The first
CPU invocation had different runtime variables and was rejected, retained, then
rerun correctly. Flagged CPU particles/GPU combined/groups wall deltas were
investigated with hash-verified retained binaries in A/B/B/A order; no consistent
slowdown repeated. All 220 selected resource/copy medians match. One matched 20 ms
process-tree RSS sample is 760,975,360 bytes versus 777,908,224 baseline; no causal
memory-saving claim is made. WGPU-only Gaussian retention remains explicitly
nonzero, bounded and distinct from those CPU/combined measurements.

Both fresh CPython 3.13 Linux wheels (workspace and extracted sdist) install and
pass clean font/audio/encoding smoke tests. Installed sdist-wheel WGPU raw frames
match CPU and encoded three-frame outputs probe correctly on software and NVIDIA.
All 143 renderer files are in the sdist; non-manifest source/assets are byte-exact.
All ten Python-reachable manifests resolve within the extracted source. No mandatory
Python dependency was added. SDK neither/CPU/WGPU checks and CLI/schema/40 examples
pass. Final just style passes; the WGPU skill's moved test destinations validate.
Ignored stress/other-platform/hosted-CI/device-memory limits remain in the report.

## Decisions and discoveries

2026-10-27: Keep the existing checkout/cache on the explicitly requested feature
branch. No additional worktree is needed. Media extraction may proceed independently
of renderer file moves; keep its diff and commit distinct.

## Completion / handoff

Four renderer crates and renderer-independent media are implemented, committed and
locally validated. See the results report for the complete evidence and deliberate
limits. Finalization records the report commit and clean-tree/protected-ref check.
No push, merge, PR or CI polling occurred. Retain active evidence through handoff;
follow PLANS.md transfer/removal before any later, separately authorized merge.
