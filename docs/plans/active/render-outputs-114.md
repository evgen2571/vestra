# Frame, preview and animation outputs
Status: in progress
Branch / baseline: `feat/render-outputs-114` / `4bcefc4` (`origin/develop`)

## Objective
Implement [#114](https://github.com/evgen2571/vestra/issues/114): frame access,
still images/sequences, selected video ranges, render overrides, previews,
GIF and animated WebP through the existing composition engine.

## Baseline and references
- The SDK's `PreparedProject` already renders random-access owned RGBA8 frames
  by time or index. Python exposes bytes and reusable prepared state.
- One-shot rendering and prepared video share the evaluated plan, backend,
  media sink, lifecycle and atomic publication infrastructure.
- Preview compilation already reduces actual render dimensions. Coordinate and
  effect scaling need validation for the new overrides. It does not currently
  reduce FPS or encoder cost.
- Python wheels resolve packaged FFmpeg as of `ce20d6c`.
- See [render pipeline](../../development/architecture/render-pipeline.md),
  [timing](../../development/architecture/project-to-plan.md), and
  [media I/O](../../development/architecture/media-io.md).

## Scope and non-goals
All scope and acceptance criteria remain owned by #114, including performance,
media timing, backend consistency, bounded memory and supported wheel checks.
No new composition engine, plugin framework, cache service, GUI, or unrelated
codec/container additions.

## Constraints and decisions
- Reuse prepared frames for still output, sequences and arrays.
- PNG/static WebP retain straight alpha; JPEG composites over black.
- NumPy access is optional, returns an independent writable contiguous RGBA8
  array, and must document shape, row stride and ownership.
- Still output uses media-owned encoders and existing publication semantics.
- Sequences return a count and release each frame before requesting the next;
  no output-path or frame collection grows with timeline length.
- Ranges must retain original project time and zero-base encoded timestamps;
  define rational rounding and synchronize trimmed audio before implementation.
- Overrides belong to runtime requests, leaving authored project settings intact.
- Keep static MP4 fast-path performance and lifecycle/error handling intact.

## Milestones
- [x] Still image export, NumPy access, selected frames and numbered sequences;
  verify exact/lossy pixel contracts, alpha, ownership and random-access timing.
- [x] Selected video ranges; verify boundaries, full-render agreement and audio
  synchronization.
- [x] Resolution/FPS/quality overrides and preview preset; preserve layout and
  effects while reducing actual rendering work.
- [x] GIF and animated WebP through bounded streaming media output;
  verify palettes, transparency, availability, timing and cleanup.
- [ ] Backend/output regression coverage, representative performance/memory
  measurements, documentation/examples and supported-wheel validation.
- [ ] Contributor checks, commits, issue checkbox update and PR to `develop`.

## Acceptance criteria
Audit every scope checkbox and completion criterion in #114 against current
tests, decoded output, benchmarks and public API/docs. CPU, software WGPU,
hardware WGPU and OS wheel evidence remain separate. Report unavailable
environments honestly; do not claim them from local Linux evidence.

## Progress and verification
- 2026-11-09: fetched latest develop and created managed isolated worktree.
- Baseline: `cargo test -p vestra --test public_sdk prepared` inside Nix passed
  8 tests. This does not establish strict adapter verification.
- Frame/still/NumPy/sequence APIs implemented with typing, docs and a runnable
  example. Initial full Python run: 1,467 passed, 42 skipped. Media crate:
  127 unit tests + 3 still tests passed, 5 ignored. Scoped all-target Clippy,
  mypy, Ruff, Rust formatting, docs links and site checks passed.
- An independent slice review found an unchecked final image-buffer flush.
  A POSIX subprocess regression reproduced false success and destination loss
  for all three formats. The fix explicitly flushes before publication;
  post-fix full verification: 1,470 Python tests passed, 42 skipped; all 130
  media tests passed (5 additional tests ignored), 8 focused prepared SDK
  tests passed, scoped all-target Clippy, mypy, Ruff, formatting, docs and site
  checks passed. Review also removed an unnecessary retained prior frame.
- The example generated PNG/JPEG/WebP, selected frames, a 24-frame sequence
  and a contiguous `(180, 320, 4)` array; inspected its poster PNG.
- Current normal-environment WGPU preparation fails with
  `WGPU device_request failed: Parent device is lost`. Strict software and
  hardware adapter validation remain unverified; investigate the established
  Nix software environment before treating this as a blocker.
- `ced4ba5` committed the verified frame/still/sequence/NumPy slice.
- Shared range groundwork: added SDK `PreparedProject::frame_range`, using
  checked core rational frame counting, and routed Python continuous sequences
  through it. New SDK boundary/fractional-rate tests were observed failing
  before implementation and then passed. Post-change verification: full Python
  suite 1,470 passed, 42 skipped; 10 focused prepared SDK tests passed;
  all-target SDK/binding Clippy, mypy, Ruff, formatting and docs links passed.
- Selected video ranges now flow through public SDK/native/Python requests into
  the staged loop. Original frame/time evaluation and audio automation are
  retained; sink indices and trimmed final audio start at zero. Static visuals
  reuse one cached frame; default full static MP4 still uses its FFmpeg image
  loop. Ranged static output currently uses the cached raw-frame loop.
- Independent review reproduced two range bugs before fixes: fresh static
  ranges submitted frame zero, and FFmpeg's `-frames:v` truncated audio for a
  final partial frame at 30000/1001 FPS. Regression tests failed before each
  fix, then passed. Selected rawvideo uses pipe EOF to bound video, allowing
  final audio padding to complete. The reviewer verified both corrections.
- Range verification: 1,492 Python tests passed, 42 skipped, plus compileall;
  full SDK/media tests 291 passed, 7 ignored; scoped all-target/all-feature
  Clippy, formatting, Ruff, public typing and docs checks passed. Exact raw CPU
  selected pixels match full output, reordered staged completions retain
  absolute indices, and audio sample/tail/automation/PTS tests pass. A direct
  whole-package mypy check reports the same 86 diagnostics as unchanged
  develop (sorted diagnostic sets match); public typing checks pass.
- Inspected actual CPU reference/decoded frames for a 24-frame selected clip:
  ffprobe confirms H.264, 320×180, 24/1 FPS, 24 frames, 1.0 seconds, PTS zero.
  Evidence: `target/range-visual-114/`; verification logs: `target/range-*-114.log`.

- 2026-11-10: runtime SDK/Python resolution, FPS and quality overrides and
  `render_preview()` wired without mutating authored data. API regressions
  were observed failing before implementation; 24 focused CPU override tests
  pass. Preview defaults cap longest side at 640, FPS at 15, and select
  Preview encoder quality; explicit values take precedence.
- GIF/WebP streaming implementation passes 130 media unit tests, seven animated
  integration tests and three still integration tests (five manual benchmarks
  ignored), all-target media Clippy, 12 public Python animation tests and a
  native SDK full/range static-cache regression. Decoded GIF/WebP/reference
  frames inspected under `target/animation-visual-114/`.
- An integration run reached 1,526 passing Python tests plus 42 skips and two
  assertions against metrics absent from Python. Those assertions now follow
  public progress counters; native tests prove one actual cached static render.
  A fresh complete suite is still required after final renderer changes.
- Strict software discovery works in `nix develop .#wgpu-software`: Vulkan
  llvmpipe (LLVM 21.1.8), Mesa 26.1.5, classification software. General-shell
  device loss is not evidence against this configured adapter. Hardware remains
  unverified.
- Memory profiles use the existing process-tree sampler at 20 ms for 300 and
  3,000 frames, 640×360/30 FPS. PNG steady RSS is flat at 45.15 MiB; long GIF
  early-to-late 94.33→96.13 MiB and WebP 95.99→99.55 MiB. Publication frame
  counts/timing and sampled pixels verified. Evidence under
  `target/benchmark-results/render-outputs-114/`; this finite profile does not
  establish multi-hour/large-file stability. Matched preview timings pending.

## Decisions and discoveries
- 2026-11-09: isolated evaluation, random access, owned pixels and preview
  spatial scaling already exist; extend those seams instead of rewriting them.
- 2026-11-09: NumPy 2.5 requires Python 3.12 and its stubs break the project's
  Python 3.11 mypy target when installed under Python 3.14. Development/test
  extras select NumPy <2.5; the optional public NumPy extra remains >=1.26.
- Next: resolve video intervals on the original rational frame grid, zero-base
  only encoder indices/audio, and share range boundary rules with sequences.
  Keep prepared state and existing staged rendering/publication paths.
- 2026-11-09: confirmed an existing preview geometry defect with actual CPU
  MP4 output: a 640×360 rectangle in a 1280×720 project occupies half the
  canvas at full resolution but the entire 640×360 preview. At normalized
  `(0.125, 0.5)`, full output is `[0, 0, 0]`, preview `[253, 0, 0]`.
  Evidence is under `target/preview-scale-baseline-114/`. Resolution overrides
  must preserve authored layout using the shared geometry infrastructure;
  reducing `plan.canvas` dimensions alone is insufficient.

- 2026-11-10: canonical schema-v1 output paths remain MP4 defaults. Animation
  destinations are runtime requests; high-level `render()` snapshots authored
  settings and carries the actual path separately. Preflight continues to
  validate all declared assets, including audio, for consistent error and
  duration behavior. Animations omit audio encoding but retain visual analysis.
- Different output aspect ratios use centered uniform fit, never stretch.
  Global effects include final-canvas borders. Existing physical effect limits
  produce explicit errors when upscaling exceeds supported kernels.
- GIF uses bounded per-frame palettes and binary alpha; WebP streams lossless
  still VP8L frames into a bounded RIFF mux rather than accumulating animation
  state. Timing quantizes cumulative boundaries to format clocks.

## Final verification and handoff
- Commits: frame exports `ced4ba5`, shared range validation `2771468`, selected
  video/audio ranges `0ad207d`. Overrides, animation, regression coverage and
  documentation are implemented. Final `just check` passes: workspace build,
  all-target/all-feature Clippy, tests, schema freshness and documentation links.
  Final strict site build checks 15,987 generated links and passes ten tests.
- Full Python suite: 1,528 passed, 42 skipped. Public typing passes. Installed
  Linux wheel passes still, sequence, range/override, preview, GIF, WebP and
  audio checks using packaged FFmpeg 7.0.2. Its clean environment omits optional
  NumPy; the development suite verifies NumPy ownership and pixel contracts.
- Strict software Vulkan: 782 core/common/CPU/WGPU library tests passed,
  22 ignored; 12 resolution/parity tests passed with actual llvmpipe execution.
  Maximum CPU/WGPU channel difference is one. Hardware GPU is unverified.
- Effects preview median 5,929→857 ms (6.92×); FFV1 decode control
  1,207→671 ms (1.80×). Full 1280×720/30 FPS/CRF23 versus preview
  640×360/15 FPS/CRF30. Same-time encoded PSNR: effects 38.14–42.73 dB,
  decode control 29.72–30.43 dB. Static MP4 medians 954→932 ms have overlapping
  sample ranges; both retain looped-image encoding and identical sampled pixels.
- Memory, exact commands, executable hashes, decoded output and timing records:
  `target/benchmark-results/render-outputs-114/README.md`. These finite CPU
  profiles do not establish hardware, multi-hour or large-file behavior.
  Forty benchmark regression tests and 53 release-helper tests passed.
- The intermittent executable-fixture `ETXTBSY` failure is fixed by creating
  the fake executable in a child process. All seven animation integration tests
  and 80 concurrent fixture repetitions pass; production code has no retry.
- Remaining: commit/push, PR to `develop`, supported
  OS wheel matrix, evidence-based issue checkbox update, and plan cleanup.
