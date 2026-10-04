set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments := true

# List available development commands.
default:
    @just --list

# Run the canonical contributor checks.
check:
    ./scripts/check.sh

# Format Rust and the maintained Python format surface.
fmt:
    cargo fmt --all
    uvx --from 'ruff>=0.12,<0.13' ruff format python/vestra/sources/base.py python/vestra/sources/__init__.py

# Check formatting without changing files.
fmt-check:
    cargo fmt --all -- --check
    uvx --from 'ruff>=0.12,<0.13' ruff format --check python/vestra/sources/base.py python/vestra/sources/__init__.py

# Run the inexpensive CI style checks.
style: fmt-check lint-python showcase-style docs-check

# Lint Rust and Python.
lint: lint-rust lint-python

# Check all Rust targets/features with warnings denied.
lint-rust:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Lint the maintained Python package.
lint-python:
    uvx --from 'ruff>=0.12,<0.13' ruff check python/vestra

# Run Rust tests; optionally pass Cargo/test arguments.
test *args:
    ./scripts/test-rust.sh "$@"

# Build/install the native Python package and development dependencies.
python-sync:
    uv sync --locked --extra dev

# Run Python tests; optionally pass pytest arguments.
python-test *args: python-sync
    uv run python -c 'import vestra'
    uv run pytest "$@"
    uv run python -m compileall -q python/vestra

# Validate the schema and compare it with generated output.
schema-check: schema-validation schema-freshness

# Validate schema inputs and canonical examples (requires Python dependencies).
schema-validation:
    uv run python crates/vestra-cli/tests/schema_validation.py

# Compare the checked-in schema with freshly generated output.
schema-freshness:
    ./scripts/check-schema.sh

# Check local public documentation links.
docs-check:
    uv run --no-project python scripts/check-docs.py

# Check the CLI help command.
cli-smoke:
    cargo run -q -p vestra-cli -- --help

# Build, install, and verify a wheel in a clean environment.
wheel-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    wheel_python=$(uv python find)
    rm -rf dist smoke-venv
    uv build --python "$wheel_python" --wheel --out-dir dist
    uv venv --python "$wheel_python" smoke-venv
    uv pip install --python smoke-venv/bin/python dist/*.whl
    smoke-venv/bin/python scripts/verify-wheel.py

# List discovered WGPU adapters.
wgpu-list:
    ./scripts/verify-wgpu.sh --list

# Run workspace and strict software-safe WGPU tests.
wgpu-software:
    ./scripts/verify-wgpu.sh --software

# Run workspace and strict hardware WGPU tests (requires a hardware adapter).
wgpu-hardware:
    ./scripts/verify-wgpu.sh --hardware

# Validate or render canonical examples; defaults to validation only.
examples *args='--validate-only':
    ./scripts/render-examples.sh "$@"

# Run a CPU smoke benchmark into a new output directory.
benchmark-smoke output:
    uv run --no-project python scripts/benchmark.py run --suite smoke --output "$1"

# Run a canonical benchmark; backend is cpu or hardware-wgpu.
benchmark output backend='cpu':
    uv run --no-project python scripts/benchmark.py run --suite canonical --output "$1" --backend "$2"

# Compare two saved benchmark suite JSON files.
benchmark-compare before after:
    uv run --no-project python scripts/benchmark.py compare "$1" "$2"

# Check showcase authoring/scripts without importing the native extension.
showcase-style:
    uvx --from 'ruff>=0.12,<0.13' ruff check examples/showcase scripts/render-showcases.py scripts/check-docs.py tests/test_showcase_assets.py tests/test_documentation_links.py
    uvx --from 'ruff>=0.12,<0.13' ruff format --check examples/showcase scripts/render-showcases.py scripts/check-docs.py tests/test_showcase_assets.py tests/test_documentation_links.py

# Reproduce the licensed footage selections and original soundtrack.
showcase-assets:
    uv run python examples/showcase/prepare-assets.py

# Render full showcase timelines cheaply with generated offline test footage.
showcase-smoke:
    uv run python scripts/render-showcases.py --smoke --offline --output-dir target/showcase-smoke

# Render all full-resolution showcases from bundled inputs.
showcase:
    uv run python scripts/render-showcases.py

# Replace committed videos/posters and the README GIF with fresh full renders.
showcase-refresh:
    uv run python scripts/render-showcases.py --preview --update-results
