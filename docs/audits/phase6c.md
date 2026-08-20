# Phase 6C closure audit

Recorded on 2026-07-29 in the workspace CI environment. This audit separates
compilation, pure/injected logic, adapter-gated invocation, runtime skips, and
real GPU execution. A passing Cargo test that emits a skip marker is not claimed
as WGPU runtime verification.

## API and JSON closure

`RenderResult` and `PreparationReport` use SDK-owned `AdapterInfo` and
`RenderPerformance`; renderer-owned DTOs are not part of the supported public
result signatures. `RenderPerformance` keeps the previous report JSON shape.
The exact-key regression covers default and populated (including WGPU-only)
values. The serialized keys are:

```text
active_item_consideration_count, advanced_effect_count, accumulation_buffer_count,
bind_group_count, bitmap_cache_hit_rate, bitmap_cache_hits,
bitmap_cache_insertions, bitmap_cache_misses, bitmap_cache_requests,
brightness_effect_count, cache_budget_bytes, cache_current_bytes,
cache_current_entries, cache_evictions, cache_oversized_entries_skipped,
cache_peak_bytes, command_submission_count,
compiled_transition_association_count, contrast_effect_count,
declared_clip_count, decoded_image_count, decoded_source_bytes,
effect_pass_count, evaluated_track_count, generated_local_effect_count,
generated_transform_contribution_count, global_effect_count, hidden_clip_count,
image_source_count, keyframe_count, local_effect_count, maximum_active_layers,
output_texture_count, parsed_colour_count, peak_cache_entries,
peak_decoded_bytes, pipeline_count, readback_buffer_bytes,
readback_buffer_count, rendered_clip_count, rendered_frame_count,
saturation_effect_count, schedule_event_count, sampler_count, shader_module_count,
solid_color_source_count, source_texture_bytes, source_texture_count,
tint_effect_count, uploaded_texture_bytes, uploaded_texture_count,
zero_frame_clip_count
```

The former `serde(skip)` fields remain absent: staging estimates, pipeline and
slot counts, in-flight/submitted/completed/written counters, poll/wait/map/repack
timings, callback queue measurements, mapping failures, flush/abort timings,
and slot-lifetime timings. These are advanced Rust diagnostics, not stable
serialized or Python-v0.1 metrics. The README documents the operation-local
semantics for one-shot, prepared-video, CPU, WGPU, caches, and timings.

The public-coordinator regression
`prepared::tests::editor_prepare_deduplicates_fallback_warning_and_keeps_its_report_immutable`
uses `Editor::prepare(... Auto)` with a narrow test-only preflight/backend seam.
It proves selected CPU retention, no adapter, matching fallback metadata, one
`VESTRA-WGPU-FALLBACK` diagnostic with its complete structured identity, stable
warning order, and an unchanged `PreparationReport` after both a frame and a
video operation.

## WGPU environment policy

Public SDK classification now skips only non-empty diagnostic sets composed
exclusively of `WGPU-ADAPTER-NOT-FOUND` or `WGPU-NO-COMPATIBLE-ADAPTER`.
The matching diagnostic is selected by code, not array index. Tests prove that
mixed adapter/project diagnostics, an empty set, and device, shader, pipeline,
and texture failures are fatal. Renderer runtime helpers apply the same strict
policy; callback/readback runtime tests now emit the same explicit marker and
fail in strict mode if no adapter is available. Device creation failures always
fail after discovery.

Adapter-gated tests emit one of:

```text
WGPU_RUNTIME_EXECUTED adapter=<name> backend=wgpu
WGPU_RUNTIME_SKIPPED reason=no-compatible-adapter ...
```

Normal mode invoked the adapter-gated bodies but emitted the skip marker because
the environment returned `WGPU-ADAPTER-NOT-FOUND`. No compatible adapter,
adapter metadata, device metadata, or real CPU/WGPU parity execution is claimed.

## Commands and results

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo check --workspace` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` | passed; SDK 54 unit + 23 public-SDK + 1 export, renderer 119, CLI unit/integration suites passed |
| `cargo test -p video-editor` | passed; 54 unit, 23 public-SDK, 1 public-export |
| `cargo test -p video-editor --test public_sdk -- --nocapture` | passed; 23 tests, four adapter-gated invocations explicitly skipped |
| `cargo test -p video-editor-render -- --nocapture` | passed; 119 tests; adapter-gated and callback runtime paths emitted explicit skips |
| `VIDEO_EDITOR_REQUIRE_WGPU=1 cargo test -p video-editor --test public_sdk -- --nocapture` | expected failure: 19 passed, 4 adapter-dependent failures due to no compatible adapter |
| `VIDEO_EDITOR_REQUIRE_WGPU=1 cargo test -p video-editor-render -- --nocapture` | expected failure: 102 passed, 17 adapter-dependent failures, including both callback queue tests, due to no compatible adapter |
| `cargo check -p video-editor --no-default-features --features cpu` | passed |
| `cargo check -p video-editor-render --no-default-features --features wgpu` | passed with seven existing CPU-only dead-code warnings |
| `python3 crates/video-editor-cli/tests/schema_validation.py` | passed |
| `./scripts/verify-public-asset.sh` | passed |

## Conclusion

Phase 6C’s source, API-contract, deterministic test, CLI/schema, and compile
closure is complete. Real WGPU execution remains environment-limited: this
runner has no compatible adapter, so strict commands correctly fail and no
runtime parity or adapter metadata is recorded. A GPU-capable runner must pass
the two strict commands before claiming real WGPU runtime verification.
