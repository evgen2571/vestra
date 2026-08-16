#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
# Mesa's WSL GL implementation permits one active EGL context at a time. Keep
# ordinary platform runs parallel, and serialize only the affected GL path.
is_wsl=false
case "$(uname -r)" in
  *microsoft*|*Microsoft*|*WSL*) is_wsl=true ;;
esac
if [[ "${VESTRA_WGPU_BACKEND:-}" == "gl" || ( "$is_wsl" == true && -z "${VESTRA_WGPU_BACKEND:-}" ) ]]; then
  cargo test --workspace --all-features -- --test-threads=1
else
  cargo test --workspace --all-features
fi
python3 crates/vestra-cli/tests/schema_validation.py
schema_tmp_dir=$(mktemp -d)
trap 'rm -rf "$schema_tmp_dir"' EXIT
cargo run -q -p vestra-cli -- generate-schema --output "$schema_tmp_dir/project.schema.json"
cmp "$schema_tmp_dir/project.schema.json" schemas/project.schema.json
