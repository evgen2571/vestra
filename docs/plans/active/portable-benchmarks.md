# Portable benchmark collection
Status: completed
Branch / baseline: `feat/portable-benchmarks` / `e3aa96436394e3e21b9b846ca5a35c479ad91d87` (`origin/develop`, fetched 2026-10-29).

## Objective
Implement the attached portable benchmark specification: one uv command prepares
source builds, collects CPU/hardware-WGPU/MoviePy/cache/optional sustained
evidence, and produces a private-path-sanitized ZIP with honest partial statuses.
No push, PR, or merge is authorized.

## Baseline and references
- Clean checkout; local develop and main refs recorded before implementation.
- Existing runner: `scripts/benchmark.py`, `benchmarks/suites.json`,
  `crates/vestra/benches/animation_effects.rs` and `benches/support/record.rs`.
- Existing public-API MoviePy workloads/verification live in
  `benchmarks/comparisons/moviepy/`; their manual requirements currently get
  removed by subsequent uv synchronization.
- `VideoDecoderOptions.cache_budget_bytes` already isolates decoded RGBA cache;
  current-frame retention remains active with budget zero.
- Windows FFmpeg build explicitly invokes sh and make with MSVC configure flags;
  release CI initializes the MSVC developer environment. Ordinary PowerShell
  without that environment is insufficient.
- Read performance/media/GPU/testing/install guides and native build/workflows.
  CodeGraph executable exits 139, so direct source navigation is used.

## Scope and non-goals
Thin orchestration around existing suites/comparisons, a benchmark-only native
decoder harness, locked optional benchmark packages, contributor documentation
and regression tests. No renderer optimization, cache redesign, public engine
API expansion, or canonical comparator replacement.

## Constraints and decisions
- Bootstrap is stdlib only and uses `uv sync --locked --extra benchmark` after
  native checks; workers use the programmatically resolved managed interpreter.
- Build and fixture preparation precede measurement; distinct timings for
  worker wall time, decoder access, internal stages, and verification.
- Hardware discovery/classification comes from Vestra; every measured hardware
  result must prove WGPU and an eligible adapter. Unavailable hardware is explicit.
- Deterministic shared moving inputs replace private footage by default; local
  footage remains optional. Stream-copy repetition prepares a two-hour source.
- Cache A/B uses the existing decoder option; hashes are computed outside each
  access timer and compared exactly. MoviePy exposes latency, not invented cache
  counters. Repeated placements are a distinct public-API workload.
- Sustained output is serial and explicit, without audio analysis; output
  verification and resource monitoring are separate from short captures.
- Results stay in target; archive contains only allowlisted evidence, with path
  sanitation. Media is excluded. Cancellation kills owned process trees.

## Milestones
- [x] Portable bootstrap, native preflight, process measurement and archive.
- [x] Versioned community suite and strict CPU/WGPU integration.
- [x] Locked MoviePy, deterministic fixtures, structural/visual verification.
- [x] Long-source validation and controlled cache matrix with MoviePy access.
- [x] Sustained workload, bounded monitoring, cancellation, output verification.
- [x] Unit/platform tests, real Linux smoke, repository checks and guide.

## Acceptance criteria
Cover all 18 acceptance items in the supplied task, including partial failures,
source/executable/input provenance, sample distributions, exact cache pixels,
two-hour source sparse access, shortened sustained end-to-end smoke, archive
privacy and unchanged established compatibility checks. Native Windows/macOS
and multi-hour endurance must be reported unverified unless actually executed.

## Progress and verification
- Investigation and branch creation complete. Implemented stdlib bootstrap and
  managed-interpreter re-entry, isolated locked benchmark extra, native build/
  codec checks, portable process trees, partial statuses, summaries and ZIP.
- Implemented community suite, opt-in native output validation, MoviePy portable
  inputs, cache harness/matrix, long-source timestamp/seek validation, public
  same-source placement companion, and optional sustained workers. Native
  end-to-end execution is pending source-build/dependency downloads.
- `uv lock --check --offline`: passed (31 locked packages, Python 3.11 split for
  compatible NumPy). No existing development dependency was upgraded.
- `uv run --no-project --python 3.13 --with pytest --with psutil==7.2.2
  --with Pillow==11.3.0 python -m pytest tests/test_benchmark_collector.py
  tests/test_benchmark_compare.py tests/test_moviepy_comparison.py
  tests/test_moviepy_visual_validation.py -q`: 97 passed, 1 explicit native E2E
  skipped. Later supporting additions still need the final rerun.
- Real 24-second stream-copy fixture regression passes after fixing B-frame
  overrun: frame-count-limited copy replaces `-t`, which retained 722 rather than
  720 frames. Independent seek pixels and all 720 timestamps are checked.
- Documentation link check: 338 targets, zero missing.
- Real missing-tool preflight and successful native preflight both produce ZIPs
  with correct exit/status, without benchmark measurements.
- Pinned Nix input download failed twice with a truncated tar archive. Local
  validation uses an existing locally cached nixpkgs source via an input override;
  flake.lock is unchanged.
  That shell has Rust 1.98.1 / FFmpeg 8.1.2. Cold Cargo FFmpeg clone and uv wheels
  are downloading slowly; retain live handles rather than restarting on timeout.
- Ruff wheel download timed out; retry uses a longer per-request timeout.
- Ruff check/format and cargo fmt now pass. Timeout descendant tests exposed a
  post-kill exit race; the sampler waits on observed owned children before
  returning. Focused timeout/CPU tests: 7 passed.
- Actual adapter discovery: Intel Iris Xe, Vulkan and GL, classified integrated
  GPU by Vestra; llvmpipe classified software. Render validation remains pending.
- Initial CPU source build failed at FFmpeg Git clone with an HTTP/2 reset/early
  EOF. The queued cache build is fetching again; CPU retry requests HTTP/1.1.
- Maturin PEP 517 accepts `--profile release`, not `--release`; corrected the
  collector and regression test. Build constraints are workspace settings and
  were ignored in the initial collector uv.toml; they now live in pyproject.toml,
  constraining isolated builds to the Maturin version already in uv.lock.
- Resumed hardening: missing Git no longer prevents preflight/ZIP creation;
  cancelled JSON/JSONL evidence is retained safely, including truncated lines;
  non-finite timeouts are rejected. Sampled sustained CPU percent and FPS are
  explicit. Collector-only tests: 41 passed, 1 native E2E skipped.
- Actual two-hour low-resolution fixture passed: 216,000 continuous CFR PTS,
  all timestamps decoded, beginning/middle/end and loop-boundary seek pixels
  match the 12-second segment exactly.
- Source review found decimal-vs-rational FPS comparison and loss of the opt-in
  validation environment control in the canonical runner. Both are corrected;
  native record tests now request and assert published-output validation.
- A second HTTP/2 FFmpeg fetch failed with reset/early EOF. Cancelled the obsolete
  duplicate development sync to let the process-local HTTP/1.1 retry proceed.
  The full collector smoke has passed native preflight and is downloading its
  locked Python environment; native render evidence is still pending.
- Further full Git transfers failed over HTTP/1.1 and FFmpeg's official server.
  Local validation now uses an exact upstream Git object cache at
  `611cec792aa54e2e53dbc1be95dcf5955d9a99ba`, fetched through bounded partial-clone
  batches and verified with git fsck. Three large wheels were downloaded in
  resumable ranges, checked against uv.lock SHA-256, then installed only with
  uv into the owned benchmark environment. No repository build settings or
  global Git configuration were changed for these network workarounds.
- Bootstrap now installs locked dependencies (including Maturin) without the
  project, then builds Vestra with that same managed interpreter/backend.
  Preserved the comparison's prior imageio-ffmpeg 0.6.0 pin. Regression run:
  107 passed, 13 native tests skipped pending the media build.
- Actual MoviePy smoke passed all three scenarios with structural/full-decode
  validation. All six MoviePy long-source access patterns passed independent
  exact FFmpeg RGB pixel checks. A broad script-only run had four expected
  import failures in code-to-video tests because Vestra was not installed;
  rerun after the native build is required.
- Strict hardware Vulkan parity: `cargo test --locked --release -p vestra-render
  --test parity --all-features` with both hardware-required controls enabled:
  70 passed, 11 ignored. Full encoded hardware collection is still pending.
- Added cancellation coverage for private native benchmark scratch and
  truncated worker evidence. Sustained terminal events preserve observed
  frame progress. Two cancelled bootstrap invocations produced incomplete ZIPs
  and reaped their owned workers.
- Full Linux hardware smoke exercised all ten native CPU and hardware Vulkan
  workloads, all nine MoviePy scenario/engine combinations, six native/MoviePy
  cache access patterns and all three sustained engines. Structural validation,
  exact cache A/B pixels and sustained finalization passed. Dense synthetic
  footage failed the existing visual tolerance; the run correctly returned a
  failed ZIP while preserving successful independent evidence.
- Portable comparison footage now uses a 640x360 canvas even in smoke and
  supersampled, lightly band-limited moving patterns. Cache/sustained smoke
  uses a separately identified 128x72 source. The visual threshold is unchanged;
  final full smoke verification is pending.
- Added regression tests that first reproduced ignored FFprobe decode errors
  and duplicate early/late RSS observations in sparse sustained series, then
  passed after correction. Phases now partition elapsed worker time; missing
  phases remain unavailable. Sustained smoke samples at 100ms.
- Latest tooling regression suite: 110 passed, one explicit native E2E skipped.
  An actual failed ZIP passed integrity, JSON parsing, allowlist/media exclusion
  and private home/checkout/username checks (359 evidence files, 358064 bytes).
- Full hardware smoke `vestra-bench-linux-20261029T182118Z-8c5a00cf` exited 0:
  ten CPU and ten strict hardware workloads, nine MoviePy scenario/engine
  combinations, 135 visual assertions, all six cache patterns, 216,000 source
  timestamps and three 90-frame sustained outputs passed. Inspected layered
  contact sheets. The ZIP passed JSON/integrity/privacy/exclusion checks.
- Native broad script regression run: 155 passed, one E2E skip, 19 subtests
  passed, including all release benchmark record integration tests. The prior
  four missing-Vestra import failures are resolved in the managed native env.
- Archive audit found MoviePy v2's observable progress bar is `frame_index`,
  not `t`. Added a regression that reproduced missing progress, then passed
  after correction; final native E2E will also assert terminal frame progress
  for both CPU engines. No renderer semantics changed.
- `just check` is running with the exact cached FFmpeg source and native Nix
  environment; debug FFmpeg compilation is still in progress.
- Final bootstrap audit found editable native extensions could be replaced by
  a later development build. The collector now always builds a fresh,
  non-editable release wheel with the managed pinned Maturin backend, bypassing
  uv's wheel cache while retaining Cargo's cache. An actual fresh-wheel build
  passed; its native module is inside the benchmark environment, not the checkout.
- `just python-test`: initial offline isolated-backend lookup failed because
  Maturin's registry wheel was absent from that cache. Retried with the already
  installed locked Maturin backend (`UV_NO_BUILD_ISOLATION=1`): 1,205 passed,
  followed by successful bytecode compilation.
- `just check`: Rust format/build and generation checks passed, but Clippy 1.98
  rejects unchanged baseline `chunks_exact` calls in shape raster/audio files
  (`chunks_exact_to_as_chunks`). Confirmed Cargo manifests/lock and every
  reported file match the base. No lint suppression or unrelated fix was made.
  Remaining workspace tests, schema and style checks are running independently.
- Updated focused tooling suite: 112 passed, one explicit E2E skip. Final native
  E2E is running against the isolated wheel and will assert MoviePy v2 progress.

## Decisions and discoveries
- 2026-10-29: clean checkout permits the requested task branch in place, preserving
  both base branches and all existing worktrees.
- 2026-10-29: portable Python can use a different libc from a Nix native compiler.
  libclang preflight now falls back to a compiled native loader probe, testing
  the ABI bindgen actually uses. Native extension imports still need a compatible
  interpreter/runtime; local validation should use the Nix shell's Python when
  overriding nixpkgs to a newer libc.
- 2026-10-29: forward native decoder jumps decode intervening frames; the cache
  workload records this existing behavior and bounds it by worker timeout.
  No decoder optimization is in scope. Eviction counters are not exposed and
  remain null; the sequence deliberately exceeds the explicit frame budget.

## Completion / handoff
Implementation and local verification are complete. The feature is delivered
on the assigned branch; no push, PR or merge is authorized. Retain this
plan for review and remove before merging per `PLANS.md`.

Final evidence:
- `just test` with Vulkan selected: 1,163 passed, 30 intentionally ignored,
  zero failures across 33 test/doc-test groups.
- `just python-test` with the installed locked build backend: 1,205 passed;
  bytecode compilation passed. Focused tooling: 112 passed, one explicit E2E
  skip; the native E2E was then enabled separately and passed.
- Native E2E `191035Z-7741d67f`: all CPU/cache/sustained tasks passed using the
  fresh isolated wheel, including actual MoviePy v2 terminal progress. ZIP:
  255 evidence files, 224,528 bytes.
- Final strict hardware companion `192841Z-9fdef985` (CPU/MoviePy/sustained
  selection): ten CPU and ten WGPU workloads, nine scenario/engine combinations,
  135 visual checks and three 90-frame sustained outputs passed. All three
  terminal progress records report 90 frames. Actual Intel Iris Xe / Vulkan /
  Mesa 26.1.8, integrated hardware; no fallback. ZIP: 262 files, 300,547 bytes.
- Both final archives passed integrity, JSON parsing, media/local-scratch
  exclusion and home/checkout/username sanitation checks. Every native cache
  request was compared exactly; MoviePy decoder opens remain unavailable and
  initial public clip opens are labeled separately.
- Rust formatting, lock freshness, schema validation/freshness, comparison
  lint/format and documentation links passed. The style retry used the verified
  local Ruff wheel after an offline cache miss. `just check` remains failed on
  the unchanged baseline Clippy 1.98 lint described above; no check was weakened.
- Native Windows/macOS and multi-hour or >4 GiB endurance are unverified. CI
  tooling contracts cover all three OSes but were not executed remotely.

These are correctness smoke captures, not performance baselines. The native
CPU E2E overlapped unrelated repository compilation; its timings must not be
used for comparative performance claims. The final hardware companion ran after
the broader checks finished.
