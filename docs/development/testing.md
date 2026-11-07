# Testing

Enter `nix develop` and run `just` to list commands. Recipes also work in an
equivalent native environment with Just, Rust, uv and the required native
dependencies installed.

## Choose checks for your change

Start with a focused check while editing. Before a pull request, run the broader
checks for the surfaces you changed; equipment-dependent results are separate
evidence.

| Change | Focused editing check | Before a pull request / additional evidence |
| --- | --- | --- |
| Documentation or site | `just docs-check` | `just site-check` and browser inspection; see [Documentation authoring](documentation.md). |
| Core model, timeline or validation | `cargo test -p vestra-core TEST_FILTER` | `just check`; schema and Python checks if public contracts changed. |
| Python authoring or bindings | `just python-test python-tests/TEST.py` | `just python-test`; rebuild the native extension after Rust changes. |
| CLI or canonical JSON | `cargo test -p vestra-cli --test cli_validation` | `just schema-check`, current CLI help and `just examples`, as applicable. |
| Effects or rendering | Source/effect-specific CPU tests and `just effect-check` | Inspect frames; add [software/hardware WGPU evidence](gpu-validation.md) for affected paths. |
| Media I/O | Focused media/SDK tests with native FFmpeg libraries | Probe encoded output and verify [sync, publication and cleanup](media-pipeline.md#verify-encoded-output). |
| Performance | Benchmark-tool tests for tooling changes | Record and compare the [canonical suite](performance.md); smoke timings are not a baseline. |
| Packaging | Focused package/API tests | `just wheel-smoke` verifies a built and installed wheel. |

## Focused editing checks

Run only the relevant crate or Python file while editing:

```bash
cargo test -p vestra-core TEST_FILTER
cargo test -p vestra-cli --test cli_validation
just python-test python-tests/TEST.py
```

`just test TEST_FILTER -- --nocapture` forwards arguments to the workspace Rust
runner. `just python-test` syncs the editable package, imports the extension,
runs pytest and then compileall. Use the repository's Nix/uv workflow for native
Python work.

**After Rust changes, rebuild the native extension explicitly.** Ordinary uv
synchronization can reuse a cached extension:

```bash
uv sync --locked --extra dev --reinstall-package vestra
just python-test python-tests/TEST.py
```

## Pre-PR contributor gate

For native code changes, build the Python package as needed, then run the local
contributor gate:

```bash
just python-sync
just check
```

`just check` runs [scripts/check.sh](https://github.com/evgen2571/vestra/blob/develop/scripts/check.sh):

- Rust formatting, including the included effect semantic sources.
- Effect-generation freshness and exporter/generator tests.
- Workspace compilation, Clippy and Rust tests with all features.
- Schema validation and generated-schema freshness.
- Public Markdown file-link checks.

It does **not** run Python package tests, Python lint, installed-wheel checks,
`just site-check`, or strict software/hardware WGPU verification. Add the checks
selected above. Documentation-only work uses `just docs-check` and
`just site-check` without building the engine.

## Command reference

| Command | Purpose |
| --- | --- |
| `just fmt` / `just fmt-check` | Format Rust and the maintained Python surface, or check without writing |
| `just lint` | Run workspace Clippy and Python lint |
| `just test` | Run workspace Rust tests with the platform's thread policy |
| `just python-test` | Sync the native package, run pytest and compileall; explicitly rebuild after Rust edits |
| `just schema-check` | Validate schema inputs and check generated-schema freshness |
| `just gen-effects` / `just gen-effects-check` | Regenerate effect artifacts, or check freshness without writing |
| `just effect-check` | Generation, core/common/CPU tests and adapter-independent WGPU library tests |
| `just wheel-smoke` | Build/install a wheel and verify its installed rendering contract |
| `just examples` | Validate canonical examples |
| `just docs-check` | Check relative file targets in public Markdown; not heading anchors |
| `just site-check` | Build the site and check generated links, anchors, media and snippet syntax |
| `just showcase-smoke` | Render/probe full timelines at 320×180 / 6 fps with offline fixtures |
| `just showcase` / `just showcase-refresh` | Render real bundled inputs; optionally update committed videos, posters and GIF |

### Canonical examples and showcase renders

`just examples` validates every canonical JSON example with the CLI. To render
one CPU representative per category, run `./scripts/render-examples.sh` without
flags; use `just examples --all` for all examples and `--output-dir DIR` to keep
artifacts. See the [examples index](https://github.com/evgen2571/vestra/blob/develop/examples/README.md)
for the category map and media-dependent example.

`just showcase-smoke` renders full timelines at low resolution with offline
fixtures. Full-resolution previews follow the [documentation media workflow](documentation-media.md).

## Backend feature isolation

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

Common tests cover shared contracts and preparation; implementation-private
tests live in the CPU and WGPU crates. Facade parity tests require both features.
The facade defaults to both backends; its no-backend build checks contracts and
unavailable-backend behavior. The adapter-list example requires WGPU.

For dependency changes, verify normal, build and development dependencies with
`cargo tree -p PACKAGE --edges normal,build,dev`:

- CPU-only builds must select neither the WGPU implementation nor WGPU libraries.
- WGPU-only builds must select no CPU implementation.
- Media must select no renderer crate, including common.

Also collect compiler artifacts in an isolated `CARGO_TARGET_DIR` build when
changing crate boundaries or dependencies. A full workspace build unifies
features and cannot prove single-backend isolation.

## Schema, CLI and effect compatibility

Use `just schema-check` for schema validation and generated-schema freshness.
CLI changes also need current help (`just cli-smoke`) and CLI integration tests.
Public Python contract changes need the rebuilt extension and relevant typing,
serialization and validation tests. Media tests require native FFmpeg development
libraries; runtime workflows may also need `ffmpeg` and `ffprobe` on `PATH`.

For effects, start with `just effect-check` and the affected pixel tests. The
[effect test reference](effect-testing.md) maps generator, frozen compatibility,
structural and renderer fixtures to their owners and explains how to preserve
independent expectations.

## Equipment-dependent WGPU verification

`just effect-check` includes adapter-free WGPU tests. Ordinary
`cargo test -p vestra-render-wgpu` includes adapter tests by default.
Run `just wgpu-software` or `just wgpu-hardware` for strict adapter verification;
follow [GPU validation](gpu-validation.md) for device selection, test scope and
what each result establishes.

## Benchmark-tool checks

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
Canonical semantic validation must remain independent of filesystem/media
availability; `ve validate` deliberately adds environment preflight. Default
timeline budgets and representability constraints are documented in
[validation layers](../reference/project-format.md#validation-layers).
Use a focused regression to reproduce the defect before changing a contract;
roadmap acceptance criteria do not describe features already available.

## Rendered sequences and references

Renderer changes need actual frame inspection as well as structural tests.
Use the [rendered-output procedure](gpu-validation.md#inspect-rendered-output)
for boundary sequences, independent references, alpha and justified tolerances.

<span id="agent-verification-tools"></span>

Agent workflow and tool checks are documented in the
[repository skill index](https://github.com/evgen2571/vestra/blob/develop/.agents/skills/README.md).

## CI coverage

The [CI workflow](https://github.com/evgen2571/vestra/blob/develop/.github/workflows/ci.yml)
runs on pull requests, pushes to `main`, and manual dispatches:

| Job | Checks | When it runs |
| --- | --- | --- |
| Style | Rust/Python formatting, Python and showcase/script lint, public Markdown links, effect generation | Every run |
| Rust CPU | Dependency isolation; common, CPU, CPU-only and no-backend facade tests | Native changes; after Style |
| Rust Integration | WGPU-only isolation/tests, workspace Clippy/tests, strict software WGPU, CLI help and schema freshness | Native changes; after Style |
| Python | Native package tests, schema validation, canonical examples, showcase smoke and script regressions | Native changes; after Style |
| Installed wheel (Python job) | Build, clean install, import/typing and short AAC render/probe | Pushes to `main` and manual runs |
| CI Success | Require successful checks; accept only the intended native-job skips | Every run |

Changes limited to Markdown, `docs/`, and `LICENSE` skip the native jobs. Any
other path triggers them, including manifests, lockfiles, fixtures, scripts and
workflows. Manual runs execute all checks. Failed or cancelled jobs fail the gate.

CI's WGPU evidence is [software correctness](gpu-validation.md#verify-software-safe-correctness).
Hardware validation and visual inspection remain separate checks.
