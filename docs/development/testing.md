# Testing

Run focused tests while changing a contract, then use the repository check for the broader local gate:

```bash
./scripts/check.sh
```

It runs `cargo fmt --all -- --check`, workspace `cargo check` and Clippy with all features, workspace Rust tests, `uv run python crates/vestra-cli/tests/schema_validation.py`, and a generated-schema comparison against `schemas/project.schema.json`. It serializes Rust tests when `VESTRA_WGPU_BACKEND=gl` or WSL's default adapter path needs one EGL context.

`./scripts/render-examples.sh --validate-only` validates every canonical JSON
example with the CLI. Without flags it also renders one CPU representative per
category into a temporary directory. Use `--all` for every example, or pass
`--output-dir DIR` to keep artifacts. This is a maintainer workflow, not part
of the normal check. See the [examples index](../../examples/README.md) for
the category map and media-dependent example.

For a fast edit loop, use `cargo test -p CRATE TEST_FILTER`, `cargo test -p vestra-cli --test cli_validation`, or `uv run pytest python-tests/TEST.py`. Native Python work uses `uv sync --locked --extra dev`, then `uv run pytest`; CI also imports the extension and runs `python -m compileall -q python/vestra`. Build the extension/package through the repository's documented Nix/uv workflow, not a separate hand-maintained environment.

Schema changes need both `ve generate-schema --output PATH` and the schema-validation test. CLI changes need current help and CLI integration tests. Rendering changes need CPU tests and source/effect-specific tests. Media tests need working native FFmpeg development libraries; runtime workflows may also need `ffmpeg`/`ffprobe` on `PATH`.

Benchmark tooling has focused tests independent of the Python extension:

```bash
uv run --no-project --with pytest pytest tests/test_benchmark_compare.py tests/test_benchmark_record.py
python scripts/benchmark.py run --suite smoke --output target/benchmark-results/smoke-check
```

The first command tests comparison contracts and skips real-render tests unless
`VESTRA_BENCH_EXECUTABLE` names the built release `animation_effects` benchmark.
Set that variable to run the full test file, including suite recording,
deterministic asset generation, frame counts, and production audio. The smoke
command builds the benchmark automatically and renders every suite workload.
Neither smoke timings nor test timings are a performance baseline. Use the
[canonical suite](performance.md) for measurements.

CI runs these categories in `nix develop .#ci`: Rust format/check/Clippy/tests, maintained Python format/lint/tests, schema contract, installed-wheel rendering smoke, and build smoke. The wheel smoke installs the built wheel into a clean environment, checks its installed import and typing marker, then renders/probes a short AAC project. The WGPU CI job is explicitly software correctness. Hardware validation is separate: discover adapters, set a chosen `VESTRA_WGPU_BACKEND`, then run `scripts/verify-wgpu.sh --hardware` and report the actual adapter. See [GPU validation](gpu-validation.md).
