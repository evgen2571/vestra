# CPU renderer

The CPU backend implements `RenderBackend` for prepared resources and evaluated frame plans. It receives decoded image/video data and renderer-independent evaluated sources. It does not parse project JSON or decide which layers are active. Its job is to turn one evaluated frame into an owned RGBA frame for a caller or media sink.

The backend prepares raster assets, font/text state, video decoder sessions and source-specific resources before the hot loop. A frame then dispatches evaluated sources, including images, video frames, shapes, text, Spectrum2D, particles and recursively evaluated groups. Geometry and crop adaptation feed tiny-skia raster/compositing work. Ordered layers and effect passes are composited into the output surface; nested compositions render their child-local work before their parent presentation is applied.

The implementation keeps reusable surfaces, decoded data, worker-local state, bitmap/static-layer caches and source resources around the prepared render. Video decode remains a media-facing session but its pixels are consumed through prepared renderer resources. Reuse is important because a frame path that repeatedly allocates a canvas, opens a file, reparses a font, or rebuilds an effect state will distort both performance and renderer parity.

`cpu/compositor.rs` keeps recursive composition, masks/mattes and cache
presentation in one `ComposeContext`, which borrows the prepared assets and
surface pools. `cpu/effects.rs` keeps ordered pass execution with its convolution
implementation. Their pixel, ordering, cache and surface-reuse tests live in
`cpu/compositor/tests.rs` and `cpu/effects/tests.rs`, alongside test-only helpers.

`particle_raster.rs` and the pixel color transform in `blend.rs` are shared
renderer utilities. WGPU's additive-particle compatibility path uses this same
rasterizer, so it is available with either backend feature. CPU/WGPU comparison
test modules require both features; WGPU planning, packing and resource-limit
tests also run in WGPU-only configurations.

Preserve these boundaries when changing CPU code:

- Source and effect dispatch must consume evaluated values and respect plan ordering.
- Every returned frame is tightly owned RGBA8 data. Callers must not borrow a mutable CPU surface after the backend advances.
- Group recursion must retain child-local timing and layer ordering.
- Caches may change cost, never visible semantics. Key them with all inputs that affect a cached result.
- Keep I/O, project validation and public-path resolution out of the per-frame loop.

The CPU backend is the direct fallback when WGPU is unavailable under an automatic preference. It is not a reduced semantic mode; a documented limitation needs explicit source/effect/backend evidence.

## Memory accounting

Resource reports describe named engine storage, not process RSS/PSS or a
whole-process limit. CPU snapshots are taken after worker work has completed.

| Resource | Report and ownership |
| --- | --- |
| Shared images, shape/text pixels and glyph atlases | `decoded_source_bytes` reports logical pixel payloads once across workers; excludes spare backing capacity, preparation temporaries and font-system internals. |
| Effect, mask, coverage and feather buffers | `cpu_scratch_buffers_retained` / `cpu_scratch_bytes_retained` include all five RGBA images and three `f32` vectors per worker, using backing capacity. |
| Nested composition and matte-save pool | Included in the scratch gauges after surfaces return to the pool; sibling compositions reuse depths. Resize replaces mismatched surfaces. |
| Crop and static-layer caches | `cache_current_bytes` covers crops; `static_cached_bytes` covers whole-layer pixels. These are logical bytes charged to their budgets, separate from scratch capacity and decoded video. |
| Native video RGBA cache | `video_cache_bytes_retained`; `video_cache_bytes_peak` sums individual session high-water marks, including retired sessions, rather than measuring one simultaneous peak. |
| Native video current frame | `video_current_frame_bytes_retained` includes capacity only when its pixels are absent from the cache. Returned `Arc` aliases do not add capacity. Evicted frames retained by callers are outside this gauge. |
| Native decoder working planes | `video_decoder_buffer_bytes_retained` sums stride-inclusive RGBA scaler and queued lookahead plane spans. It excludes buffer tail padding, codec reference/reorder frames, packets, demux/scaler internals and other opaque native allocations. Custom decoder providers that do not measure memory leave video byte gauges zero. |
| Output payloads | `cpu_full_frame_allocations` counts cumulative output-buffer allocations. `cpu_output_frame_bytes_estimated` allows two tight RGBA payloads per worker in the SDK scheduler; it does not track arbitrary caller-held frames after transfer. A static final-frame template also has SDK ownership. |

Scratch allocation/reuse counts are cumulative backend events; operation results
report their deltas. Scratch byte/buffer fields are current capacities, and cache
peaks are per-owner high-water marks. They cannot be added as if they were
simultaneous peaks. Per-frame matte-cache images and group-mask owner/group
temporaries are released before the idle snapshot; the worker estimate includes
their structural storage allowance. Container bookkeeping, prepared plan copies,
thread stacks, allocator-retained freed memory, audio preparation and encoder
processes are outside these pixel-storage gauges. Use external process sampling
for resident memory; see [performance](../performance.md).

Dynamic automatic worker selection retains the existing 512 MiB heuristic,
one reserved CPU when possible, and maximum eight workers. Its per-worker
`cpu_worker_memory_estimate_bytes` is calculated from:

- Eight eager scratch buffers and two SDK output payloads at output dimensions.
- Distinct retained composition pool depths reachable through visible groups
  and active matte chains, the peak simultaneous matte-cache images, and maximum
  overlapping group-mask owner/group temporaries. Hidden sources are reached
  through active consumers; unused hidden and out-of-output layers add no storage.
- Three source-sized RGBA images per independent video cursor: current, incoming
  converted pixels and scaler output. Per-asset peak overlapping source clocks
  cover extra cursors; serial clips reuse the primary, which remains retained
  after the asset leaves view. Matching local origins and source start/rate share
  a cursor. Group and source-mask intervals use composition-local offsets and
  parent/output clipping. This conservatively ignores opacity, frame sampling and cache
  hits, and sums each asset's peak. Native row padding, YUV/lookahead and opaque
  codec state are excluded from the estimate.

All estimate arithmetic saturates on overflow and selection always permits at
least one worker. Shared decoded assets and configured cache budgets are
separate; the 512 MiB heuristic is not a total-memory guarantee. Static visual
plans still use one worker. Corrected accounting alone does not release memory;
any concurrency change needs paired throughput and process-memory measurements.
