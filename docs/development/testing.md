# Testing

Enter `nix develop` and run `just` to list the available commands. Recipes use
the active environment, so they also work in an equivalent native setup with
Just, Rust, uv, and the required native dependencies installed.

Run focused tests while changing a contract, then use the repository check for the broader local gate:

```bash
just python-sync
just check
```

`just check` delegates to `scripts/check.sh`. It runs `cargo fmt --all -- --check`, explicit rustfmt checks for included effect semantic sources, effect-generation freshness and generator tests, workspace `cargo check` and Clippy with all features, workspace Rust tests, `uv run python crates/vestra-cli/tests/schema_validation.py`, and a generated-schema comparison against `schemas/project.schema.json`. The shared `scripts/test-rust.sh` runner serializes Rust tests when `VESTRA_WGPU_BACKEND=gl` or WSL's default adapter path needs one EGL context.

| Command | Purpose |
| --- | --- |
| `just fmt` / `just fmt-check` | Format Rust and the maintained Python surface, or check without writing |
| `just lint` | Run workspace Clippy and Python lint |
| `just test` | Run workspace Rust tests with the platform's thread policy |
| `just python-test` | Sync/build the native package, run pytest, and compileall |
| `just schema-check` | Validate schema inputs and check generated-schema freshness |
| `just gen-effects` / `just gen-effects-check` | Regenerate effect artifacts, or check freshness without writing |
| `just effect-check` | Generation, core/common/CPU tests and adapter-independent WGPU library tests |
| `just wheel-smoke` | Build/install a wheel and verify its installed rendering contract |
| `just examples` | Validate canonical examples |
| `just docs-check` | Check relative file targets in public Markdown |
| `just showcase-smoke` | Render/probe full timelines at 320×180 / 6 fps with offline fixtures |
| `just showcase` / `just showcase-refresh` | Render real bundled inputs; optionally update committed videos, posters and GIF |

`just examples` delegates to `scripts/render-examples.sh --validate-only` and
validates every canonical JSON example with the CLI. Running
`./scripts/render-examples.sh` without flags also renders one CPU representative
per category into a temporary directory. Use `just examples --all` to render
every example, or `just examples --output-dir DIR` to keep rendered artifacts.
Canonical validation and showcase smoke rendering also run in the Python CI job.
Full-resolution rendering remains an explicit maintainer workflow. See the [examples index](../../examples/README.md) for
the category map and media-dependent example.

For a fast edit loop, use `cargo test -p CRATE TEST_FILTER`, `cargo test -p vestra-cli --test cli_validation`, or `just python-test python-tests/TEST.py`. `just test` forwards Cargo/test arguments, such as `just test TEST_FILTER -- --nocapture`. Native Python work uses `just python-sync`, then `just python-test`; the test recipe syncs dependencies itself, imports the extension, and runs compileall after pytest. Build the extension/package through the repository's documented Nix/uv workflow, not a separate hand-maintained environment.

After Rust-only changes, explicitly rebuild the editable native package before
Python tests: ordinary uv synchronization can reuse a cached extension.

```bash
uv sync --locked --extra dev --reinstall-package vestra
just python-test
```

Check renderer packages and facade feature selections independently:

```bash
cargo test -p vestra-render-common
cargo test -p vestra-render-cpu
cargo test -p vestra-render-wgpu
cargo test -p vestra-render --no-default-features
cargo test -p vestra-render --no-default-features --features cpu
cargo test -p vestra-render --no-default-features --features wgpu
cargo test -p vestra-render --no-default-features --features cpu,wgpu
cargo test -p vestra-media
```

Common tests cover shared contracts/preparation. CPU and WGPU packages contain
their implementation-private tests; cross-backend parity suites live in facade
integration tests and require both features. WGPU-only coverage includes planning,
parameter, upload and resource-limit checks. The facade defaults to both backends;
its no-backend build preserves contracts and unavailable-backend behavior. The
adapter-list example requires the WGPU feature.

Use `cargo tree -p PACKAGE --edges normal,build,dev` for actual package activation.
CPU-only tests must select neither the WGPU implementation nor WGPU libraries;
WGPU-only tests must select no CPU implementation. Media's graph contains no
renderer crate, including common. For extraction/dependency changes, also collect
compiler artifacts from an isolated `CARGO_TARGET_DIR` build: a full workspace
build unifies features and cannot prove single-backend isolation.

Schema changes need both `ve generate-schema --output PATH` and the schema-validation test. CLI changes need current help and CLI integration tests. Rendering changes need CPU tests and source/effect-specific tests. Media tests need working native FFmpeg development libraries; runtime workflows may also need `ffmpeg`/`ffprobe` on `PATH`.

Effect generation checks require only the core metadata exporter and Python's
standard library; they do not build renderers or the native Python extension.
`just gen-effects-check` checks committed-artifact freshness and runs the core
exporter and stdlib generator tests. To run the Python generator tests separately
and then check the frozen public contracts:

```bash
uv run --no-project python -m unittest discover -s tests -p test_effect_generation.py
just python-test python-tests/test_effect_generation_compatibility.py
```

The latter compares imports, signatures, inheritance, serialization, validation
timing and JSON behavior with frozen pre-generation fixtures. It explicitly
preserves the Ascii schema/parser mismatch. See the
[fixture instructions](../../python-tests/fixtures/effects/README.md) to repeat
the same comparisons against the exact baseline revision. Do not refresh the
expected values from generated output to make a regression pass.

Effect declarations and static semantic forwarding are described in
[adding effects](adding-effects.md). Whole-catalog property tests compare typed
visitors and compiled constraints with descriptors, exercise negative keyframe
validation for scalar/plain/point tracks, and check declared active intervals.
Frozen specimens additionally
pin old serde/compile/evaluate/topology and transition/normalization behavior.
Renderer tests pin old kernel order/private bits, shader labels/composed bytes
and packed-value specimens. Registry-driven WGPU tests validate all registered
records against WGSL, including named offsets, scalar types and typed uploads.
These structural/specimen tests complement actual CPU/software/hardware frame
tests; they do not establish exhaustive numerical correctness or hardware parity.

`just effect-check` runs the WGPU library with `--no-default-features`. The
crate's default `adapter-tests` feature includes tests that initialize a graphics
instance, request a device, submit commands or read back pixels. Disabling it
keeps registration, shader parsing/composition, parameter ABI/packing, resource
calculations and pure planning tests, including tests with `gpu_` in their names.
The test-only instance factory rejects initialization in this mode, so an
accidental adapter request fails before graphics discovery. No Vulkan, EGL,
software ICD or working adapter is required for the focused command.

`just wgpu-software` and `just wgpu-hardware` explicitly enable `adapter-tests`
and run the entire private WGPU library suite under strict adapter requirements,
including readback callback validation and instance/queue lifetime tests.
The facade's strict rendering/parity tests remain included. Ordinary
`cargo test -p vestra-render-wgpu` retains adapter tests through default features;
hardware validation still requires a proven hardware adapter. See
[GPU validation](gpu-validation.md) for selecting and verifying the actual device.

Benchmark tooling has focused tests independent of the Python extension:

```bash
uv run --no-project --with pytest pytest tests/test_benchmark_compare.py tests/test_benchmark_record.py
just benchmark-smoke target/benchmark-results/smoke-check
```

The first command tests comparison contracts and skips real-render tests unless
`VESTRA_BENCH_EXECUTABLE` names the built release `animation_effects` benchmark.
Set that variable to run the full test file, including suite recording,
deterministic asset generation, frame counts, and production audio. The smoke
command builds the benchmark automatically and renders every suite workload.
Neither smoke timings nor test timings are a performance baseline. Use the
[canonical suite](performance.md) for measurements.

The task-specific local workflows are indexed in
[Vestra skills](../../.agents/skills/README.md); their visual tools also have
four isolated self-tests. `just docs-check` checks public Markdown file targets,
not skill metadata, links under `.agents/` or Markdown anchors. Validate those
explicitly when changing skills, and execute the existing tool self-tests.

## Timeline and media checks

Use [project-to-plan](architecture/project-to-plan.md#time-domains-and-frame-boundaries)
for time domains/duration authority and
[media output verification](media-pipeline.md#verify-encoded-output) for probe,
AV-sync and failure criteria. Prefer existing seams:

- Core timeline/validation/compiler tests for rational arithmetic, half-open
  activity, transition endpoints and nested schedules.
- SDK project preflight and public prepared-frame tests for media-resolved
  duration, exact frame access, diagnostics, owned pixels and output lifecycle.
- High-level Python project/group/video tests for omitted durations and lowering;
  schema/typing/CLI tests when public contracts change.
- Media cursor/cache/VFR and sink/output tests for timestamp selection, EOF,
  ordered writes, failure cleanup and destination publication.

For each boundary, distinguish authored end, effective clipping and source EOF;
test adjacent frames/timestamps and exclusive output end at rational rates.
Pure validation must remain independent of filesystem/media availability.
Use a focused regression to reproduce the defect before changing a contract;
roadmap acceptance criteria do not describe features already available.

## Rendered sequences and references

Compare a short deterministic sequence before/at/after effect and transition
boundaries, with nested offsets, masks/mattes and fractional FPS as relevant.
Repeat timestamps in forward/backward order and fresh/prepared sessions to
expose request-order state or cache invalidation. Retain per-frame metrics and
contact sheets; a sequence average can hide a bad boundary frame.

Backend parity is one reference, not independent proof: also use expected
literal pixels, a known fixture/reference or a cache-disabled path as suitable.
Inspect transparent hidden RGB, alpha, semitransparent edges and composition
over a visible backdrop. Justify tolerances for the operation and encoded/raw
format rather than copying broad thresholds. Fix seeds/fonts/assets/time and
investigate nondeterminism instead of widening tolerances. Keep CPU, strict
software-WGPU and named hardware-WGPU evidence separate.

## CI coverage

CI has four validation jobs followed by the required `CI Success` gate:

- Style runs `just style` in `nix develop .#ci` to check Rust formatting, the maintained Python format/lint surface, public documentation file links, showcase/script formatting and lint, and `just gen-effects-check`. The generation check compiles only the core exporter and runs stdlib generator tests. Rust and Python jobs start only after it passes.
- Rust CPU checks normal/build/dev dependency activation, then tests common, CPU
  and the CPU/no-backend facade in `nix develop .#ci`. Its Rust cache has a separate
  CPU key; its dependency guard must select no WGPU package.
- Rust Integration first checks and tests WGPU-only packages before workspace
  feature unification, then runs `just lint-rust` and `just wgpu-software` in
  `nix develop .#wgpu-software`. The verification script runs workspace tests once,
  followed by strict WGPU-private and facade integration tests. Clippy replaces
  separate compilation-check jobs. `just cli-smoke schema-freshness` checks CLI
  help and generated-schema freshness in the same job.
- Both Rust CI jobs disable debug symbols in Cargo's `dev` and `test` profiles to keep build artifacts within the hosted runner's disk budget. Debug assertions and the local development profiles remain unchanged.
- Python runs `just python-test` and `just schema-validation` in `nix develop .#wgpu-software`, sharing the native extension build for pytest, schema validation, compileall,
  canonical-example validation and showcase smoke rendering. Script regression tests
  cover documentation links, asset checksum/cache behavior and benchmark comparison/recording. The software Vulkan shell makes WGPU tests strict and supplies the adapter they require. Pushes to main and manual runs additionally run `just wheel-smoke` in `nix develop .#ci` to build a wheel, install it in a clean environment, verify its import and typing marker, and render/probe a short AAC project. PRs defer this packaging check until main.

Rust dependencies/build artifacts, uv downloads, and Nix builds are cached. Rust cache keys use the actual Nix toolchain and flake inputs; Python also includes its dependency inputs. CPU and integration cache keys are separate. The independent CPU runner duplicates some cold core/preparation compilation, and explicit WGPU-only tests add work before integration. Job separation establishes isolation; it does not by itself reduce total CI resource use. Cold runs still need to populate the caches.

Changes limited to Markdown, `docs/`, and `LICENSE` skip both Rust jobs and the Python job while Style and `CI Success` still run. All other paths (including manifests, locks, fixtures, scripts, and workflows) trigger all three native jobs. Manual runs always execute every check. The gate rejects failed/cancelled jobs and allows skips only when change detection explicitly excludes native validation.

CI WGPU validation is explicitly software correctness. Hardware validation is separate: discover adapters, set a chosen `VESTRA_WGPU_BACKEND`, then run `scripts/verify-wgpu.sh --hardware` and report the actual adapter. See [GPU validation](gpu-validation.md).
