# Renderer extraction — implementation and validation report

Status: implementation and required local validation complete, 2026-10-27.
See the [execution plan](render-crates.md) and
[approved amendments](render-extraction-review.md#l-approved-phase-3-amendments-2026-10-27).
Historical Phase 2 measurements remain unchanged. Evidence below is Phase 3.
Raw commands, exit statuses, artifacts and measurements are retained locally under
`target/render-crates/`; canonical suites remain under `target/benchmark-results/`.

## 1. Branch, revisions and commits

Branch: `refactor/render-crates`, stacked directly on completed Phase 2 `69e8eb6`.
Tested implementation/CI/package checkpoint:
`07d609601c96f6f53f589c5e21e13d3c9c3ed213`. Subsequent changes only record evidence
and correct the WGPU skill's test destinations. The final response records this
report's commit SHA and the final clean-tree check.

| Commit | Change |
| --- | --- |
| `ed35977` | Record approved amendments before implementation |
| `21c4448` | Separate CPU profiling, surface composition and host CRT |
| `e9f7c45` | Decouple media sinks and migrate SDK/test/benchmark callers |
| `3aaa33b` | Extract shared contracts/preparation and backend-owned policy |
| `1ee8c39` | Extract CPU and relocate facade parity tests |
| `4b20d3c` | Extract WGPU/shaders and complete facade feature contracts |
| `07d6096` | Migrate CI, strict discovery, documentation and packaging inputs |

All Phase 2 commits are preserved. No push, merge, PR or CI polling was performed.

## 2. Actual four-crate layout

| Crate | Actual responsibility |
| --- | --- |
| `vestra-render-common` | Lifecycle/types, immutable prepared Arc assets, geometry, necessary shared pixel/particle/numerical helpers, snapshots, generic bounded cache, kernel IDs/mapping/requirements; font and license |
| `vestra-render-cpu` | CPU workers, execution, compositing/effects, mutable worker caches, profiling, host CRT, CPU capability validation and BlueNoise include table |
| `vestra-render-wgpu` | WGPU implementation, shaders/pipelines, resources, parameters/planning, uploads/readback, GPU capability validation and private tests |
| `vestra-render` | Selection/fallback, compatibility reexports, facade feature tests and cross-backend integration tests |

Permanent [crate ownership](../../development/architecture/crates.md) and renderer,
effect, media, extension, testing and GPU guides describe the resulting boundaries.
There is no fifth renderer, contracts or parity-testing crate.

## 3. Before/after dependency graphs

These arrows represent dependencies, not runtime calls:

```text
Before: SDK -> render (CPU + WGPU implementations) -> core
        SDK -> media -> render (defaults disabled)
                       -> core

After:  SDK -> render facade -> render-common -> core
                            -> render-cpu -> render-common, core
                            -> render-wgpu -> render-common, core
                 facade also -> core
        SDK -> media -> core
```

Other existing non-renderer media dependencies remain. Backend crates depend on
neither their sibling nor facade/media/SDK, including development dependencies.

Actual unique normal/build packages for facade neither/CPU/WGPU/both are
`143/144/184/185`, versus Phase 2 `142/142/182/182`. The added packages are the
requested workspace ownership units. Media normal/build/dev packages decrease
from **173 to 65**; its normal/build graph alone has 60 packages.

## 4. Ownership decisions and approved amendments

- Common exposes borrowed prepared-resource getters, with resource fields and
  preparation builders kept private across the crate boundary. Geometry exposes
  required scalar records; logical preparation bounds and forward construction
  stay private. Necessary internal bridges are documented/hidden, without treating
  doc(hidden) as visibility enforcement.
- Kernel IDs, operation mapping and requirements stay common. CPU/WGPU own their
  support functions and facade calls them explicitly. Existing diagnostic codes,
  messages, categories and fallback stages remain. CPU support stays broad.
- Gaussian floating weights are unconditional. Calculations, integer weights,
  radius normalization and the existing 16-entry thread-local FIFO eviction remain.
  No `cpu-kernels` feature, second cache or separate Gaussian implementation exists.
- The generic LRU implementation remains shared because both backends use it;
  execution/cache instances remain backend-owned. Shared assets retain their Arc
  ownership; no image-buffer conversion/copy was added to cross these boundaries.
- Pure shared test literals live in `tests/render_effect_cases.rs`, included by
  each test owner without a backend development-dependency edge. No GPU-private
  structure or production test hook was exposed to make facade tests compile.
- Media moved earlier in its own commit. Test relocation accompanied CPU extraction
  because the old cfg(test) synchronous helpers cannot cross a crate boundary.

These are migration-order choices within the approved design, not additional
architecture. No effect, shader, core model, encoder, worker heuristic or memory
scheduling algorithm changed. Independent read-only review found no blocking
production issue; its evidence-wording correction was applied.

## 5. CPU/WGPU isolation evidence

`cargo tree --locked --edges normal,build,dev` confirms common/CPU select no WGPU
implementation/stack or Naga, and WGPU selects no CPU implementation. All four
facade selections have saved normal/build and normal/build/dev trees.

Fresh task-specific CARGO_TARGET_DIR builds retain JSON compiler artifacts:

- CPU test compilation selects common, CPU and facade, with no WGPU/Naga artifact.
- WGPU test compilation selects common, WGPU and facade, with no CPU artifact.
- Warm repeats select the same packages and rebuild no renderer package.
- Backend edits rebuild that backend and facade. Common edits rebuild common and
  the selected backend/facade. No sibling implementation is activated.

`compilation-verified.json` validates actual artifact sets, including Cargo's
path-package ID format, rather than relying on manifests or an empty name search.
Defaults remain CPU+WGPU; the no-backend facade remains usable.

## 6. Media isolation evidence

Media has **zero renderer dependencies** in normal, build and development graphs.
Its current graph also contains no cosmic-text, fontdb or tiny-skia. Image's
remaining direct pixel representation dependency is retained.

A fresh isolated media `cargo test --no-run --message-format=json` succeeds.
Both cold and warm compiler artifact sets contain no renderer/preparation-font
package. This proves actual compilation isolation in addition to the saved tree.
Media package tests pass: **112 passed, 4 ignored**.

## 7. Facade and renderer compatibility

| Command/configuration | Passed | Ignored |
| --- | ---: | ---: |
| `cargo test -p vestra-render-common` | 63 | 3 |
| `cargo test -p vestra-render-cpu` | 175 | 6 |
| `cargo test -p vestra-render-wgpu` | 142 | 4 |
| Facade `--no-default-features` | 4 | 0 |
| Facade `--no-default-features --features cpu` | 4 | 0 |
| Facade `--no-default-features --features wgpu` | 2 | 0 |
| Facade `--no-default-features --features cpu,wgpu` | 2 unit + 70 integration | 11 |

Package commands used `--locked` after manifest/lock migration. Feature tests
retain unavailable-backend diagnostics, CPU selection and CPU-only/no-backend
Auto behavior. Root/module imports and nominal types use compatibility reexports;
RenderBackend's lifecycle, owned CompletedFrame and backend fallback remain.
Workspace and strict runtime checks provide default both-backend evidence.

## 8. Rust, Python, JSON, SDK and CLI

- `just check`: **1,149 Rust passes**, including six explicit exporter tests;
  30 ignored, zero failures. Formatting, workspace check/all-target Clippy with
  warnings denied, generation freshness, 12 generator tests, schema and docs pass.
- Forced native extension rebuild: `uv sync --locked --extra dev
  --reinstall-package vestra`, completed successfully in 126.69 seconds.
- Fresh-extension Python: **1,205 passed, zero skipped** on NVIDIA and again
  **1,205 passed, zero skipped** on llvmpipe; compileall passes. CPython 3.13.14.
- `cargo check -p vestra --no-default-features`, then CPU and WGPU selections:
  all pass. Full workspace tests cover SDK/CLI integration and lifecycle paths.
- `just cli-smoke schema-check examples`: passes; **40 canonical examples** valid.
  Generated JSON/effect artifacts and frozen compatibility values are unchanged.

Public SDK/Python/JSON behavior was preserved; no new Python runtime dependency
or generated effect registry was introduced.

## 9. Migrated coverage and strict GPU execution

All original named parity tests and assertions remain. The facade owns 81 tests
(70 run, 11 opt-in ignored). Private planning/capacity/particle-target/metrics
assertions remain WGPU-owned; four ignored resource companions retain assertions
split from their existing facade pixel cases. The assertion inventory accounts
for stylization 144/144, crop 33/33, resources 66/66 and parity 56/56, including
one equivalent shared-case callback adaptation.

The strict wrapper now separately collects WGPU-private `--lib gpu_` tests and
facade `--tests --all-features gpu_` tests:

| Runtime | Actual adapter | Strict results |
| --- | --- | --- |
| Software Vulkan | llvmpipe, LLVM 21.1.8, Mesa 26.1.5, software/CPU | Workspace passes; 28 private/4 ignored + 70 facade/11 ignored; seven explicit hardware-conformance skips |
| Hardware Vulkan | NVIDIA GTX 1650 SUPER, NVIDIA 595.71.05, discrete GPU | Workspace passes; same private/facade counts, no runtime skip markers; encoded CLI comparison 1 passed |

Both `just wgpu-software` and `just wgpu-hardware` pass. General workspace tests
unset strict requirement flags; targeted execution scopes them correctly.
No test was disabled and no tolerance/golden was relaxed. Thirty workspace
ignored occurrences include four split companions versus Phase 2's 26.

## 10. Golden frames and rendering parity

**54 unique raw RGBA frames are byte-identical to Phase 2**, totaling 12,441,600
compared bytes: 18 each from CPU/software/NVIDIA across canonical timing,
geometric masks and ASCII. Repeated/backward requests remain stable.
Official mask/ASCII comparisons at zero tolerance report MAE=0, maximum channel
error=0 and zero mismatched pixels. The current contact sheet was visually
inspected. This before/after evidence is separate from existing CPU/WGPU literal
and tolerance-based parity tests, which pass on actual adapters.

All 36 WGSL files, both embedded font/license files and the CPU BlueNoise table
match Phase 2 bytes. No reference image, shader order or numerical algorithm changed.

## 11. Encoder and FrameSink migration

FrameSink now accepts `write_frame(frame_number: u64, rgba: &[u8])`. Media declares
no replacement CompletedFrame/DTO. SDK borrows the existing owned completion
through each call; scoped writer threads still join while that frame is alive.
Submission/write overlap, ordering, cancellation and error adaptation stay SDK-owned.

FfmpegSink keeps ordering/length validation, exact errors, stdin writes, failure
cleanup, finalization/process lifecycle and publication. Codec/quality/pixel-format/
audio behavior is unchanged. Static template copies and their metrics/drop timing
are retained; this refactor does not optimize them away.

Every sink/caller in media, SDK, tests and benchmarks migrated. The focused new
test proves accepted borrowed subslices and caller-buffer reuse; it does not
assert encoded pixels. Existing encoder/SDK/CLI tests and clean-wheel rendering
supply actual encoding/audio/publication evidence. Failure, abort, cancellation,
cleanup and staged/static paths remain covered by the existing suites.

## 12. CPU and NVIDIA benchmark comparison

Same host/toolchain/FFmpeg, canonical 1280x720, one warmup and five measured
samples, serial workloads. CPU retains eight workers; NVIDIA retains three
in-flight slots. Values are wall medians in milliseconds, not frame-only latency.

| Workload | CPU before | CPU after | NVIDIA before | NVIDIA after |
| --- | ---: | ---: | ---: | ---: |
| single_video | 1319 | 1398 | 702 | 721 |
| mixed_dynamic | 1988 | 2061 | 853 | 819 |
| video_heavy | 5303 | 5519 | 8951 | 8958 |
| combined | 5536 | 5497 | 945 | 1076 |
| masks | 291 | 264 | 401 | 425 |
| mattes | 255 | 229 | 487 | 493 |
| nested_groups | 2050 | 2075 | 596 | 682 |
| particles | 257 | 302 | 408 | 421 |
| blend_modes | 635 | 627 | 632 | 660 |
| production_edit | 10570 | 10522 | 3035 | 3150 |

The first CPU capture used a different runtime-variable prefix. The existing
comparer rejected it; raw data was retained and CPU was rerun under the original
software-shell variables with strict requirements unset. Both final comparisons
pass the unchanged workload/environment/backend compatibility checks.

Flagged wall deltas (CPU particles +17.5%, GPU combined/nested groups about +14%)
were investigated with retained hash-verified baseline/candidate executables in
counterbalanced A/B/B/A order, five samples and a warmup per invocation:

| Workload | First paired change | Reverse-order paired change |
| --- | ---: | ---: |
| CPU particles | -4.2% | -0.7% |
| NVIDIA combined | -4.5% | +7.1% |
| NVIDIA nested_groups | -11.9% | -1.6% |

The flagged slowdown does not repeat consistently. No algorithm/optimization was
changed in response. These local samples show variation and do not establish a
general speedup or identical performance. Original/raw stage distributions and
binary/source provenance are retained; A/B record Git metadata describes runtime
workspace state, while the sidecar identifies the actual retained binaries.

## 13. Memory, ownership and cache behavior

All **220 selected median resource/copy counters match** across matched CPU and
NVIDIA suites, including decoded payload, worker memory, scratch, cache budgets,
decoder sessions, frame copies, encoder pushes and GPU texture/pipeline/readback
counts. Worker policy, Arc sharing, decoder ownership, GPU lifetimes and cleanup
remain unchanged in source and focused lifecycle/resource tests.

One unwarmed production-edit process-tree RSS sample, including fixture generation
and owned FFmpeg children, used existing psutil 7.2.2 sampling every 20 ms:
777,908,224 bytes before versus 760,975,360 after (about 742 versus 726 MiB).
This sampled lower-bound peak is not total allocation volume, GPU residency or
proof of memory savings from extraction.

WGPU-only Gaussian retention is **nonzero**: at most 16 * 65 * 8 = 8,320 floating
coefficient bytes per populated thread, plus Vec headers (384 bytes on this
64-bit host), allocator/container overhead and padding. Calculations already
allocated those vectors temporarily; their retention changes. CPU and combined
configurations already retained floats. The canonical WGPU benchmark uses both
features like Phase 2, so it does not measure the isolated WGPU-only RSS delta.
The unchanged cache is bounded at 16 entries with the original eviction policy.

## 14. Cold/warm/incremental compilation

Rust/Cargo 1.95.0, Ryzen 5 3600, pinned Nix, CARGO_BUILD_JOBS=2, original test/dev
profiles. Fresh per-configuration target directories provide cold Cargo-artifact
builds; registry/Nix/OS page caches were retained. No existing cache was deleted.
Before uses `69e8eb6`; after uses `07d6096`, in separate Git source archives.

Seconds per invocation:

| Selection | Cold before/after | Warm before/after | Backend edit before/after | Common edit before/after |
| --- | --- | --- | --- | --- |
| CPU | 130.388 / 131.793 | 0.599 / 1.236 | 11.508 / 5.732 | 6.945 / 10.268 |
| WGPU | 171.438 / 178.167 | 0.664 / 0.699 | 17.281 / 20.609 | 10.902 / 19.499 |
| Media after only | 270.692 | 0.529 | — | — |

Controlled nonsemantic source edits were confined to the archived copies and
restored; actual workspace code was untouched. Compiler JSON confirms each
rebuild set. After selects common/backend/facade test packages; before selects
the monolith, so measurements include relocated/additional contract/private
tests and more test binaries. These single local samples are not pure production
compile benchmarks or hosted CI cost estimates. Splitting improves neither
every edit path nor total compilation automatically.

## 15. Wheel and source distribution

Fresh working-tree and extracted-sdist wheels built for CPython 3.13 Linux x86_64
and installed into separate clean environments. Both pass `verify-wheel.py`:
site-packages import/version/typing/licenses, bundled-font ASCII rendering,
H264/AAC output and early/late tone verification.

The sdist contains all four renderer manifests/sources, all 36 WGSL files,
common font/license, CPU include table, shared Rust test cases, schemas/exporter
input and vendored FFmpeg build inputs. Required assets have exact checked hashes.
All 143 renderer files are present; every non-manifest source/asset is byte-identical
to the tested working tree.
All ten Python-reachable workspace manifests resolve inside the extracted source.
Cargo dependency artifacts were reused during wheel builds; isolated compilation
was established separately above. Wheel metadata has no mandatory Python dependency.

The installed sdist-built wheel additionally renders CPU/WGPU-identical raw solid
frames and publishes/probes three-frame 32x32 H264 videos on actual software and
NVIDIA Vulkan. The task used dedicated output/venv directories for equivalent
wheel-smoke checks, preserving any existing root dist/smoke environment.

## 16. CI and documentation

CI adds one grouped CPU job with its own Rust cache and actual graph guards.
Integration tests WGPU-only activation before workspace feature unification;
full workspace/Python/style/generation/software-strict coverage remains.
CI Success includes the added job. Documentation-only skips/manual behavior
are preserved. Strict private/facade targets and GL serialization are explicit.

Shell syntax, ShellCheck, fake-Cargo routing/strict-scope/thread checks and
22 result-gate cases pass. Actual software/NVIDIA wrappers pass. Permanent docs,
generator destinations, font notices and the WGPU skill's iteration commands
follow the new owners. Generator path self-checks and skill validation pass.
Final `just style` also passes after the report and skill update.

The extra CPU runner duplicates some common/core/preparation cold work across
isolated caches. More code-generation/test units can increase common/GPU edit
costs, as the local measurements show. Hosted CI/billing was not run or measured;
no reduction in total CI cost is claimed.

## 17. Limits and possible Phase 4 work

Unverified: standard ignored stress/exhaustive/full-resolution opt-ins, other
graphics APIs/drivers/platforms, the full CPython/platform release matrix,
sustained/device-memory profiling and hosted CI resource costs. Software results
are not hardware evidence; local Vulkan hardware results cover the named NVIDIA
adapter. Benchmark and sampled-memory results retain their stated limits.

Phase 4 may separately generate backend registrations from the canonical effect
catalog while retaining typed core semantics and independent backend policy.
Static-copy removal would need its own tested accounting/performance change.
Neither registration generation nor worker/memory/FFmpeg redesign was implemented.

## 18. Repository handoff

Protected references remain at their original values:

- `main`: `b23c985a623efea14511e25aeb1df93b495beecb`
- `develop`: `c26f52f4c41a7d010775c8da61df59bf7caa8cb5`
- `refactor/render-boundaries`: `69e8eb6`

The existing sole checkout/worktree was retained. Finalization commits this
report and the execution plan, then verifies clean status on the task branch;
the final response supplies that SHA. No PR was opened or merged. Keep this
evidence through handoff, following PLANS.md transfer/removal before any future,
separately authorized merge.
