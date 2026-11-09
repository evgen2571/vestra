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
- [ ] Resolution/FPS/quality overrides and preview preset; preserve layout and
  effects while reducing actual rendering work.
- [ ] GIF and animated WebP through bounded streaming media output;
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

## Completion / handoff
Frame export slice committed as `ced4ba5`; shared range validation as `2771468`.
Selected video ranges implemented and verified. Next implement runtime
resolution/FPS/quality overrides and correct preview geometry through the shared
CPU/WGPU geometry machinery. Preview improvements, animated images, performance/memory measurements,
strict adapter checks, supported wheels, final contributor gate, issue update
and PR remain. No issue checkboxes updated or PR opened yet.
