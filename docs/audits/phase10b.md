# Phase 10B - static render reuse

Phase 10A remains the cacheability authority. A layer enters this cache only
when its compiled `content_dependency` is `Static`; renderers do not inspect
tracks, effects, frame numbers, or pixels to make that decision.

## Boundary and ownership

The cache holds a complete layer-local output after source rasterization,
transform, and local effects, but before blend-mode composition. Opacity stays
at composition time, which preserves the existing normal and destination-aware
blend implementations. Global/post effects are not cached.

`PreparedProject` owns one selected backend, so each backend owns its cache for
that prepared lifetime. A new prepared project or backend creates an empty
cache. Entries use the compiled-plan layer index, which is unique within the
immutable prepared plan. They are never keyed by frame number. Failed
submission does not insert a new WGPU entry; WGPU publishes an entry only when
the producing readback slot and generation complete, and CPU inserts only after
a complete surface has been built. A completed entry remains available across
later operations and random-access frames on the same prepared project.

WGPU tracks pending entries in a typed `PendingStaticLayers` value: a
`BTreeMap<(slot, generation), Vec<PendingStaticLayer>>`, a `BTreeSet` of
reserved keys, and `reserved_bytes`. Before it creates a cache texture, it
computes `width * height * 4`, evicts retained LRU entries if needed, and
reserves that amount. The invariant is `retained_bytes + pending_reserved_bytes
<= maximum_cache_bytes`. A key already pending renders normally for that frame;
it creates neither another cache texture nor another reservation. Completion
releases the reservation and publishes the immutable texture. Abort or failed
submission clears every incomplete reservation and key before the prepared
backend becomes terminal. These are deterministic logical RGBA8 byte estimates,
not physical VRAM residency measurements.

CPU entries are `Arc<RgbaImage>` and later composition only reads them. WGPU
entries are `Arc<StaticLayerTexture>` with an `Rgba8Unorm` texture and view,
created with `TEXTURE_BINDING | COPY_DST`; misses copy the completed working
texture directly on the GPU and hits sample that texture directly. Final-frame
readback remains unchanged.

Each backend uses the existing `maximum_cache_bytes` independently for its
static-layer cache. CPU crop caching also receives that configured limit, so it
is not an aggregate renderer-memory or VRAM cap. CPU and WGPU both estimate an
entry as `width * height * 4`. CPU checks the budget before cloning the layer
surface. WGPU checks it before creating a cache texture. Oversize entries render
normally and increment the bypass counter. Phase 10B deliberately does not add
a pool or public cache controls.

Backend static-cache counters are lifetime totals. `RenderSummary.performance`
reports hit, miss, bypass, cache-population render, and physical static-layer
execution counters as operation deltas. `static_cache_entries` and
`static_cached_bytes` are gauges sampled at the end of the operation.

`static_cache_population_renders` counts executions selected to populate a
cache entry. `static_layers_rendered` counts every physical static-layer
execution: cache populations, duplicate normal rendering while the same WGPU
key is pending, and budget bypasses. It was previously population-only; it was
not renamed, but is now defined by its name. A cache hit increments neither.

## Focused evidence

`cpu::backend::tests::reuses_complete_static_layer_surfaces_without_mutating_them`
proves one miss, one hit, one entry, one static-layer render, and equal output
across two frames. `wgpu::frame_plan::tests::static_layer_store_then_reuse_stays_before_destination_composition`
proves the planned boundary. The adapter-aware WGPU test
`gpu_reuses_static_layer_texture_without_readback_when_an_adapter_is_available`
compiles and verifies one miss, one hit, one entry, one static render, equal
pixels, and only the final readback copy on the hit frame when a compatible
adapter is available; runtime execution was unavailable in this environment.

`static_cache_matches_the_dynamic_reference_path` compares cached static output
against the normal dynamic renderer path. `dynamic_layers_bypass_the_whole_layer_cache` proves a Dynamic layer creates no
whole-layer cache request or entry. `over_budget_static_layers_render_without_retention`
proves a one-byte budget keeps output correct while recording two bypasses and
retaining no surface.
`static_layer_renders_once_across_one_hundred_frames` records exactly one miss,
99 hits, and one static-layer render.
`static_layer_activity_does_not_affect_its_cache_identity` proves inactive
frames neither request nor invalidate a static layer's cached content.
`distinct_static_layers_do_not_alias_or_mutate_under_dynamic_composition`
proves plan-local layer indices keep same-sized layers separate and later
dynamic blending cannot modify either cached surface.
`cache::tests::oversized_insert_does_not_create_the_value` proves the CPU
pre-check does not execute its clone closure. The pending reservation test
proves a key and its bytes are released both at completion and abort. Adapter
dependent WGPU cache tests remain gated when no adapter is available.

`application::render::tests::prepared_operations_report_cache_deltas_and_reuse_static_layers`
uses one CPU `PreparedProject` with one static layer active for 60 frames. The
first operation reports exactly one miss, 59 hits, one population render, one
physical static execution, one entry, and 16,384 cached bytes. The second
reports zero misses, 60 hits, zero population or physical executions, and the
same end-of-operation gauges. Its output bytes equal the first operation; the
backend lifetime counters equal the sum of both operation deltas. Random frame
50, then 5, then 30 also match the first operation after cache population.

`wgpu::resource_gpu_tests::gpu_abort_clears_pending_static_cache_reservations_when_an_adapter_is_available`
submits a real cache population, observes one pending key and its logical bytes,
then aborts and observes zero pending keys, zero pending bytes, and no retained
entry. The backend is terminal after abort by existing lifecycle design.
`gpu_multi_key_pending_cache_reservations_stay_within_budget_when_an_adapter_is_available`
uses three keys, two in-flight frames, and a two-texture budget. It reserves A
and B (460,800 bytes) and bypasses C on both frames: no retained bytes plus
pending bytes exceeds the budget; abort clears both reservations. The existing
same-key in-flight test retains one reservation and one population for two
submissions, while now recording two physical executions.

The Phase 10A compiler tests
`transform_contribution_dependency_includes_half_open_activity_interval`,
`timed_static_effects_are_dynamic_when_their_interval_changes_content`, and
`compiler_separates_static_clip_content_from_timeline_and_transition_animation`
cover the transition, partial-transform, and partial-effect cases. Phase 10B
consumes their `Dynamic` result through its only cache gate, so those layers
cannot enter the whole-layer cache. Static post-effect parameters are likewise
not cached because this phase has no full-frame cache boundary.

Phase 10C pooling/copy work and Phase 10D benchmarking are deferred. More
granular static prefixes inside dynamic layers are also deferred because the
current compiled layer plan does not expose a safe backend-neutral boundary.

## Verification status

On CPython 3.13.5, `cargo fmt --all -- --check`, `cargo check --workspace`,
strict workspace Clippy, and `cargo test --workspace` passed. The focused
render suite has 135 passing tests. Schema validation, `maturin develop
--release`, `python -m pytest python-tests -q` (259 passed, 4 adapter-gated
skips), mypy, and stubtest passed. `maturin build --release` produced the
manylinux CPython-3.13 wheel. An isolated virtual environment installed that
wheel, imported `video_editor` and `video_editor.authoring` from the wheel,
verified `py.typed`, validated a project, prepared CPU rendering from the
public API, and rendered the same frame twice with identical bytes.

This Linux environment has no compatible WGPU adapter (`WGPU-ADAPTER-NOT-FOUND`),
so the adapter-backed abort, multi-key in-flight budget, cache texture reuse,
and no-readback runtime tests were structurally compiled but returned early at
runtime. They are not `#[ignore]` or test-harness skipped tests.
Adapter-independent WGPU planning, pending-state, readback, and shader tests
ran in the 135-test render suite.
