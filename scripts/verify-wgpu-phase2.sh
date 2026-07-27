#!/usr/bin/env bash
set -euo pipefail

: "${VIDEO_EDITOR_WGPU_BACKEND:=vulkan}"
export VIDEO_EDITOR_WGPU_BACKEND
export VIDEO_EDITOR_REQUIRE_WGPU=1

echo "WGPU Phase 2 strict verification (backend: ${VIDEO_EDITOR_WGPU_BACKEND})"
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
cargo test --workspace --all-features
cargo run -- render examples/projects/effects-ready-v1.json \
  --render-backend wgpu --output "$temp_dir/render.mp4" --overwrite

if [[ "${VIDEO_EDITOR_RUN_BENCHMARKS:-0}" == 1 ]]; then
  for resolution in '320 180' '720 1280' '1920 1080'; do
    read -r width height <<<"$resolution"
    for scenario in gaussian_large glow sharpen directional_blur zoom_blur motion_blur combined global_post; do
      VIDEO_EDITOR_BENCH_BACKEND=wgpu VIDEO_EDITOR_BENCH_SCENARIO="$scenario" \
        VIDEO_EDITOR_BENCH_WIDTH="$width" VIDEO_EDITOR_BENCH_HEIGHT="$height" \
        cargo bench --bench animation_effects -- --nocapture
    done
  done
fi
