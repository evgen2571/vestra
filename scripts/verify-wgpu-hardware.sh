#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${VESTRA_WGPU_BACKEND:-}" ]]; then
  echo "VESTRA_WGPU_BACKEND must be set after adapter discovery (for example: gl)" >&2
  exit 2
fi

echo "WGPU HARDWARE verification (requested backend: ${VESTRA_WGPU_BACKEND})"
echo "Adapter discovery:"
adapters=$(cargo run -q -p vestra-render --example wgpu_adapters --all-features)
printf '%s\n' "$adapters"
if ! grep -Eq "backend=${VESTRA_WGPU_BACKEND} .*classification=(discrete_gpu|integrated_gpu)" <<<"$adapters"; then
  echo "requested WGPU backend did not expose a classified hardware adapter" >&2
  exit 1
fi

# The complete workspace includes negative backend-selection tests that must
# be allowed to simulate an unavailable adapter. Serialize GL on WSL, then
# apply the strict hardware requirement only to adapter-dependent tests.
echo "Running serialized workspace correctness tests"
cargo test --workspace --all-features -- --test-threads=1

echo "Running strict hardware adapter-dependent correctness tests"
export VESTRA_REQUIRE_HARDWARE_WGPU=1
cargo test --lib -p vestra-render --all-features gpu_ -- --nocapture --test-threads=1
cargo test -p vestra-cli --test render_regressions -- --nocapture --test-threads=1
