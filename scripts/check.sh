#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

echo "Checking Rust formatting, build, lints, tests, and schema freshness"
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
./scripts/test-rust.sh
uv run python crates/vestra-cli/tests/schema_validation.py
./scripts/check-schema.sh
uv run --no-project python scripts/check-docs.py
echo "Local contributor checks passed"
