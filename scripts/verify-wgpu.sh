#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

usage() {
  cat <<'EOF'
usage:
  scripts/verify-wgpu.sh --list
  VESTRA_WGPU_BACKEND=BACKEND scripts/verify-wgpu.sh --software
  [VESTRA_WGPU_BACKEND=BACKEND] scripts/verify-wgpu.sh --hardware

--list reports every adapter that Vestra/WGPU can discover.
--software requires an explicitly selected supported backend and verifies WGPU
correctness without making a hardware claim.
--hardware selects the first discovered hardware backend when unset, or checks
the requested backend. It rejects every non-discrete/non-integrated adapter.
EOF
}

mode=${1:-}
if [[ $# -ne 1 ]]; then
  usage >&2
  exit 2
fi

echo "Discovering Vestra/WGPU adapters"
adapters=$(cargo run -q -p vestra-render --example wgpu_adapters --all-features)
printf '%s\n' "$adapters"

if [[ "$mode" == "--list" ]]; then
  exit 0
fi
if [[ "$mode" != "--software" && "$mode" != "--hardware" ]]; then
  usage >&2
  exit 2
fi

backend=${VESTRA_WGPU_BACKEND:-}
if [[ -z "$backend" && "$mode" == "--hardware" ]]; then
  backend=$(sed -nE 's/^backend=([^ ]+) .*classification=(discrete_gpu|integrated_gpu) .*/\1/p' <<<"$adapters" | sed -n '1p')
  if [[ -z "$backend" ]]; then
    echo "hardware validation unavailable: discovery found no proven hardware adapter" >&2
    exit 1
  fi
  export VESTRA_WGPU_BACKEND="$backend"
  echo "Selected hardware backend: $backend"
fi
if [[ -z "$backend" ]]; then
  echo "VESTRA_WGPU_BACKEND must name a discovered backend for $mode" >&2
  exit 2
fi

backend=${backend,,}
case "$backend" in
  vulkan|gl|gles|metal|dx12|browser_webgpu) ;;
  *)
    echo "unsupported VESTRA_WGPU_BACKEND value: $backend" >&2
    exit 2
    ;;
esac
selected_backend=$backend
if [[ "$selected_backend" == "gles" ]]; then
  selected_backend=gl
fi
selected_adapter=$(grep -E "^backend=${selected_backend} " <<<"$adapters" | sed -n '1p' || true)
if [[ -z "$selected_adapter" ]]; then
  echo "requested backend '$backend' exposed no adapter in discovery" >&2
  exit 1
fi

if [[ "$mode" == "--hardware" ]]; then
  selected_adapter=$(grep -E "^backend=${selected_backend} .*classification=(discrete_gpu|integrated_gpu) " <<<"$adapters" | sed -n '1p' || true)
  if [[ -z "$selected_adapter" ]]; then
    echo "hardware validation unavailable: '$backend' has no proven hardware adapter" >&2
    exit 1
  fi
  export VESTRA_REQUIRE_HARDWARE_WGPU=1
  export VESTRA_REQUIRE_WGPU=1
  echo "Hardware adapter candidate: $selected_adapter"
  echo "Running serialized workspace correctness tests"
  if [[ "$selected_backend" == "gl" ]]; then
    cargo test --workspace --all-features -- --test-threads=1
  else
    echo "Skipping the GL-specific encoded-render regression for backend '$backend'"
    cargo test --workspace --all-features -- \
      --test-threads=1 \
      --skip strict_wgpu_canonical_render_matches_cpu_encoded_frames
  fi
  echo "Running strict hardware adapter-dependent tests"
  cargo test --lib -p vestra-render --all-features gpu_ -- --nocapture --test-threads=1
  echo "Hardware WGPU verification passed for backend '$backend'"
  exit 0
fi

export VESTRA_REQUIRE_WGPU=1
unset VESTRA_REQUIRE_HARDWARE_WGPU
echo "WGPU software correctness verification using: $selected_adapter"
if [[ "$selected_backend" == "gl" ]]; then
  cargo test --workspace --all-features -- --test-threads=1
else
  cargo test --workspace --all-features -- \
    --skip strict_wgpu_canonical_render_matches_cpu_encoded_frames
fi
echo "Software-safe WGPU verification passed for backend '$backend'"
