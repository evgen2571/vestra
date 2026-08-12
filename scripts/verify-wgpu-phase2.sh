#!/usr/bin/env bash
set -euo pipefail

: "${VESTRA_WGPU_BACKEND:=vulkan}"
export VESTRA_WGPU_BACKEND
export VESTRA_REQUIRE_WGPU=1

echo "WGPU Phase 2 strict verification (backend: ${VESTRA_WGPU_BACKEND})"
echo "The render result below includes the selected adapter metadata."
temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
cargo test --workspace --all-features
for test_name in \
  gpu_effect_catalogue_matches_cpu_on_the_rgba_fixture_when_an_adapter_is_available \
  gpu_matches_cpu_for_every_blend_mode_and_alpha_case_on_the_rgba_fixture \
  gpu_matches_cpu_for_generated_preset_transition_camera_shake_and_flash_frames \
  gpu_flash_matches_cpu_for_opaque_and_global_post_effect_variants \
  gpu_composite_matches_cpu_for_sizing_transforms_effects_and_alpha \
  gpu_canonical_timeline_frames_match_cpu_within_two_channels; do
  cargo test --lib --all-features "$test_name" -- --nocapture
done

for project in \
  examples/projects/effects-ready-v1.json \
  examples/transitions/zoom-blur.json \
  examples/presets/heavy-impact.json \
  examples/projects/animation-effects.json; do
  stem=$(basename "$project" .json)
  cargo run -p vestra-cli -- render "$project" \
    --render-backend wgpu --output "$temp_dir/$stem.mp4" --overwrite \
    --format json --progress json
done

if [[ "${VESTRA_RUN_BENCHMARKS:-0}" == 1 ]]; then
  for resolution in '320 180' '720 1280' '1920 1080'; do
    read -r width height <<<"$resolution"
    for scenario in gaussian_large glow sharpen directional_blur zoom_blur motion_blur chromatic_aberration vignette color_adjust blend_modes combined global_post; do
      VESTRA_BENCH_BACKEND=wgpu VESTRA_BENCH_SCENARIO="$scenario" \
        VESTRA_BENCH_WIDTH="$width" VESTRA_BENCH_HEIGHT="$height" \
        cargo bench -p vestra --bench animation_effects -- --nocapture
    done
  done
fi
