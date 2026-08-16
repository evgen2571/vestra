#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${VESTRA_WGPU_BACKEND:-}" ]]; then
  echo "VESTRA_WGPU_BACKEND must be set after adapter discovery (for example: gl)" >&2
  exit 2
fi

export VESTRA_REQUIRE_WGPU=1
export VESTRA_REQUIRE_HARDWARE_WGPU=1
echo "WGPU hardware verification (backend: ${VESTRA_WGPU_BACKEND})"
exec "$(dirname "$0")/verify-wgpu-phase3.sh"
