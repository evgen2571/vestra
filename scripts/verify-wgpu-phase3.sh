#!/usr/bin/env bash
set -euo pipefail

: "${VIDEO_EDITOR_WGPU_BACKEND:=vulkan}"
export VIDEO_EDITOR_WGPU_BACKEND
export VIDEO_EDITOR_REQUIRE_WGPU=1

echo "WGPU Phase 3 strict verification (backend: ${VIDEO_EDITOR_WGPU_BACKEND})"
echo "A software adapter validates correctness only. This script does not claim hardware performance."

cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --benches --all-features
cargo test --workspace --all-features
python3 crates/video-editor-cli/tests/schema_validation.py
scripts/verify-public-asset.sh

# Adapter-independent lifecycle gates must pass before adapter-dependent work.
for test_name in \
  ready_frames_are_returned_before_wait_for_one_blocks \
  engine_cancellation_after_submission_discards_in_flight_work \
  lifecycle_rejects_stale_and_invalid_transitions \
  abort_is_idempotent_and_restores_capacity; do
  cargo test --lib --all-features "$test_name" -- --nocapture
done

for depth in 1 2 3; do
  VIDEO_EDITOR_WGPU_IN_FLIGHT="$depth" cargo test --lib --all-features \
    gpu_pipeline_depths_produce_identical_ordered_frames_when_an_adapter_is_available \
    -- --nocapture
done

for test_name in \
  gpu_readback_preserves_padded_rows_when_an_adapter_is_available \
  gpu_resources_are_reused_across_frames_when_an_adapter_is_available \
  gpu_pipeline_depths_produce_identical_ordered_frames_when_an_adapter_is_available \
  gpu_effect_catalogue_matches_cpu_on_the_rgba_fixture_when_an_adapter_is_available \
  gpu_matches_cpu_for_every_blend_mode_and_alpha_case_on_the_rgba_fixture; do
  cargo test --lib --all-features "$test_name" -- --nocapture
done

temp_dir=$(mktemp -d)
trap 'rm -rf "$temp_dir"' EXIT
for project in \
  examples/projects/effects-ready-v1.json \
  examples/transitions/zoom-blur.json \
  examples/presets/heavy-impact.json \
  examples/projects/animation-effects.json; do
  stem=$(basename "$project" .json)
  cargo run -p video-editor-cli -- render "$project" \
    --render-backend wgpu --output "$temp_dir/$stem.mp4" --overwrite \
    --format json --progress json
done

if [[ "${VIDEO_EDITOR_RUN_BENCHMARKS:-0}" == 1 ]]; then
  for depth in 1 2 3; do
    for resolution in '320 180' '720 1280' '1920 1080'; do
      read -r width height <<<"$resolution"
      for scenario in \
        basic_composition gaussian_large glow sharpen directional_blur zoom_blur \
        multiple_layers global_post short_sequence long_sequence; do
        VIDEO_EDITOR_WGPU_IN_FLIGHT="$depth" \
        VIDEO_EDITOR_BENCH_BACKEND=wgpu \
        VIDEO_EDITOR_BENCH_SCENARIO="$scenario" \
        VIDEO_EDITOR_BENCH_WIDTH="$width" \
        VIDEO_EDITOR_BENCH_HEIGHT="$height" \
          cargo bench -p video-editor --bench animation_effects -- --nocapture
      done
    done
  done
fi
