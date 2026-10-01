#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

schema_tmp_dir=$(mktemp -d)
trap 'rm -rf "$schema_tmp_dir"' EXIT
cargo run -q -p vestra-cli -- generate-schema --output "$schema_tmp_dir/project.schema.json"
cmp "$schema_tmp_dir/project.schema.json" schemas/project.schema.json
