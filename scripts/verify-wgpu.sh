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
--software selects the first discovered backend when unset and verifies WGPU
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
if [[ -z "$backend" ]]; then
  if [[ "$mode" == "--hardware" ]]; then
    backend=$(sed -nE 's/^backend=([^ ]+) .*classification=(discrete_gpu|integrated_gpu) .*/\1/p' <<<"$adapters" | sed -n '1p')
  else
    backend=$(sed -nE 's/^backend=([^ ]+) .*/\1/p' <<<"$adapters" | sed -n '1p')
  fi
  if [[ -z "$backend" ]]; then
    echo "${mode#--} validation unavailable: discovery found no usable adapter" >&2
    exit 1
  fi
  echo "Selected ${mode#--} backend: $backend"
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

run_workspace_tests() {
  echo "Running general workspace correctness tests without strict WGPU flags"
  if [[ "$selected_backend" == "gl" ]]; then
    env -u VESTRA_REQUIRE_WGPU \
      -u VESTRA_REQUIRE_HARDWARE_WGPU \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test --workspace --all-features -- --test-threads=1
  else
    env -u VESTRA_REQUIRE_WGPU \
      -u VESTRA_REQUIRE_HARDWARE_WGPU \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test --workspace --all-features
  fi
}

run_targeted_wgpu_tests() {
  local hardware_requirement=${1:-0}
  if [[ "$hardware_requirement" == "1" && "$selected_backend" == "gl" ]]; then
    VESTRA_REQUIRE_WGPU=1 \
      VESTRA_REQUIRE_HARDWARE_WGPU=1 \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test -p vestra-render --lib --all-features gpu_ -- --nocapture --test-threads=1
  elif [[ "$hardware_requirement" == "1" ]]; then
    VESTRA_REQUIRE_WGPU=1 \
      VESTRA_REQUIRE_HARDWARE_WGPU=1 \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test -p vestra-render --lib --all-features gpu_ -- --nocapture
  elif [[ "$selected_backend" == "gl" ]]; then
    env -u VESTRA_REQUIRE_HARDWARE_WGPU \
      VESTRA_REQUIRE_WGPU=1 \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test -p vestra-render --lib --all-features gpu_ -- --nocapture --test-threads=1
  else
    env -u VESTRA_REQUIRE_HARDWARE_WGPU \
      VESTRA_REQUIRE_WGPU=1 \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test -p vestra-render --lib --all-features gpu_ -- --nocapture
  fi
}

if [[ "$mode" == "--hardware" ]]; then
  selected_adapter=$(grep -E "^backend=${selected_backend} .*classification=(discrete_gpu|integrated_gpu) " <<<"$adapters" | sed -n '1p' || true)
  if [[ -z "$selected_adapter" ]]; then
    echo "hardware validation unavailable: '$backend' has no proven hardware adapter" >&2
    exit 1
  fi
  echo "Hardware adapter candidate: $selected_adapter"
  run_workspace_tests
  echo "Running strict hardware adapter-dependent renderer tests"
  run_targeted_wgpu_tests 1
  echo "Running strict hardware CLI render regression"
  if [[ "$selected_backend" == "gl" ]]; then
    VESTRA_REQUIRE_WGPU=1 \
      VESTRA_REQUIRE_HARDWARE_WGPU=1 \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test -p vestra-cli --test render_regressions --all-features \
        strict_wgpu_canonical_render_matches_cpu_encoded_frames -- --nocapture --test-threads=1
  else
    VESTRA_REQUIRE_WGPU=1 \
      VESTRA_REQUIRE_HARDWARE_WGPU=1 \
      VESTRA_WGPU_BACKEND="$backend" \
      cargo test -p vestra-cli --test render_regressions --all-features \
        strict_wgpu_canonical_render_matches_cpu_encoded_frames -- --nocapture
  fi
  echo "Hardware WGPU verification passed for backend '$backend'"
  exit 0
fi

echo "Software WGPU correctness validation using: $selected_adapter"
run_workspace_tests
echo "Running targeted WGPU tests with a software adapter permitted"
run_targeted_wgpu_tests 0
echo "Software-safe WGPU verification passed for backend '$backend'"
