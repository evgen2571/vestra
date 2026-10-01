#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

# Mesa's WSL GL implementation permits one active EGL context at a time. Keep
# ordinary platform runs parallel, and serialize only the affected GL path.
is_wsl=false
case "$(uname -r)" in
  *microsoft*|*Microsoft*|*WSL*) is_wsl=true ;;
esac
if [[ "${VESTRA_WGPU_BACKEND:-}" == "gl" || ( "$is_wsl" == true && -z "${VESTRA_WGPU_BACKEND:-}" ) ]]; then
  RUST_TEST_THREADS=1 cargo test --workspace --all-features "$@"
else
  cargo test --workspace --all-features "$@"
fi
