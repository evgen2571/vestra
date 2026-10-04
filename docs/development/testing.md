# Testing

Enter `nix develop` and run `just` to list the available commands. Recipes use
the active environment, so they also work in an equivalent native setup with
Just, Rust, uv, and the required native dependencies installed.

Run focused tests while changing a contract, then use the repository check for the broader local gate:

```bash
just python-sync
just check
```

`just check` delegates to `scripts/check.sh`. It runs `cargo fmt --all -- --check`, workspace `cargo check` and Clippy with all features, workspace Rust tests, `uv run python crates/vestra-cli/tests/schema_validation.py`, and a generated-schema comparison against `schemas/project.schema.json`. The shared `scripts/test-rust.sh` runner serializes Rust tests when `VESTRA_WGPU_BACKEND=gl` or WSL's default adapter path needs one EGL context.

| Command | Purpose |
| --- | --- |
| `just fmt` / `just fmt-check` | Format Rust and the maintained Python surface, or check without writing |
| `just lint` | Run workspace Clippy and Python lint |
| `just test` | Run workspace Rust tests with the platform's thread policy |
| `just python-test` | Sync/build the native package, run pytest, and compileall |
| `just schema-check` | Validate schema inputs and check generated-schema freshness |
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

Schema changes need both `ve generate-schema --output PATH` and the schema-validation test. CLI changes need current help and CLI integration tests. Rendering changes need CPU tests and source/effect-specific tests. Media tests need working native FFmpeg development libraries; runtime workflows may also need `ffmpeg`/`ffprobe` on `PATH`.

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

CI has three validation jobs followed by the required `CI Success` gate:

- Style runs `just style` in `nix develop .#ci` to check Rust formatting, the maintained Python format/lint surface, and public documentation file links, and showcase/script formatting and lint. Rust and Python jobs start only after it passes.
- Rust runs `just lint-rust`, then `just wgpu-software` in `nix develop .#wgpu-software`. The WGPU recipe delegates to the verification script, which runs workspace tests once, followed by strict renderer WGPU tests. Clippy replaces separate compilation-check jobs. `just cli-smoke schema-freshness` checks CLI help and generated-schema freshness in the same job.
- The Rust CI job disables debug symbols in Cargo's `dev` and `test` profiles to keep build artifacts within the hosted runner's disk budget. Debug assertions and the local development profiles remain unchanged.
- Python runs `just python-test` and `just schema-validation` in `nix develop .#wgpu-software`, sharing the native extension build for pytest, schema validation, compileall,
  canonical-example validation and showcase smoke rendering. Script regression tests
  cover documentation links, asset checksum/cache behavior and benchmark comparison/recording. The software Vulkan shell makes WGPU tests strict and supplies the adapter they require. Pushes to main and manual runs additionally run `just wheel-smoke` in `nix develop .#ci` to build a wheel, install it in a clean environment, verify its import and typing marker, and render/probe a short AAC project. PRs defer this packaging check until main.

Rust dependencies/build artifacts, uv downloads, and Nix builds are cached. Rust cache keys use the actual Nix toolchain and flake inputs; Python also includes its dependency inputs. Cold runs still need to populate the caches.

Changes limited to Markdown, `docs/`, and `LICENSE` skip Rust and Python jobs while Style and `CI Success` still run. All other paths (including manifests, locks, fixtures, scripts, and workflows) trigger both native jobs. Manual runs always execute every check. The gate rejects failed/cancelled jobs and allows skips only when change detection explicitly excludes native validation.

CI WGPU validation is explicitly software correctness. Hardware validation is separate: discover adapters, set a chosen `VESTRA_WGPU_BACKEND`, then run `scripts/verify-wgpu.sh --hardware` and report the actual adapter. See [GPU validation](gpu-validation.md).
