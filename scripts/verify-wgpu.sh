#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${VESTRA_WGPU_BACKEND:-}" ]]; then
  echo "VESTRA_WGPU_BACKEND must be set after adapter discovery (gl or vulkan)" >&2
  exit 2
fi

export VESTRA_REQUIRE_WGPU=1
unset VESTRA_REQUIRE_HARDWARE_WGPU
echo "WGPU SOFTWARE correctness verification (requested backend: ${VESTRA_WGPU_BACKEND}; hardware not required)"
echo "Adapter discovery:"
cargo run -q -p vestra-render --example wgpu_adapters --all-features
if [[ "${VESTRA_WGPU_BACKEND}" == "gl" ]]; then
  cargo test --workspace --all-features -- --test-threads=1
else
  cargo test --workspace --all-features
fi
