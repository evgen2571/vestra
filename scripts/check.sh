#!/usr/bin/env bash
set -euo pipefail

cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
# Mesa's WSL GL implementation permits one active EGL context at a time;
# serialize the workspace tests so hardware-WGPU validation is deterministic.
cargo test --workspace --all-features -- --test-threads=1
python3 crates/vestra-cli/tests/schema_validation.py
schema_tmp_dir=$(mktemp -d)
trap 'rm -rf "$schema_tmp_dir"' EXIT
cargo run -q -p vestra-cli -- generate-schema --output "$schema_tmp_dir/project.schema.json"
cmp "$schema_tmp_dir/project.schema.json" schemas/project.schema.json
