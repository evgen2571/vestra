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
| Effect, mask, coverage and feather buffers | `cpu_scratch_buffers_retained` / `cpu_scratch_bytes_retained` count only allocated RGBA images and `f32` vectors per worker, using backing capacity. One current image is always retained; optional storage allocates on first use. |
| Nested composition and matte-save pool | Included in the scratch gauges after surfaces return to the pool; sibling compositions reuse depths. Resize replaces mismatched surfaces. |
| Crop and static-layer caches | `cache_current_bytes` covers crops; `static_cached_bytes` covers whole-layer pixels. These are logical bytes charged to their budgets, separate from scratch capacity and decoded video. |
| Native video RGBA cache | `video_cache_bytes_retained`; `video_cache_bytes_peak` sums individual session high-water marks, including retired sessions, rather than measuring one simultaneous peak. |
| Native video current frame | `video_current_frame_bytes_retained` includes capacity only when its pixels are absent from the cache. Returned `Arc` aliases do not add capacity. Evicted frames retained by callers are outside this gauge. |
| Native decoder working planes | `video_decoder_buffer_bytes_retained` sums stride-inclusive RGBA scaler and queued lookahead plane spans. It excludes buffer tail padding, codec reference/reorder frames, packets, demux/scaler internals and other opaque native allocations. Custom decoder providers that do not measure memory leave video byte gauges zero. |
| Output payloads | `cpu_full_frame_allocations` counts cumulative output-buffer allocations. `cpu_output_frame_bytes_estimated` allows two tight RGBA payloads per worker in the SDK scheduler; it does not track arbitrary caller-held frames after transfer. A static final-frame template also has SDK ownership. |

Effect passes allocate a destination before execution and reuse the existing
logical resource bindings. Masks first allocate their local image, transformed
image and coverage vector; positive feathering adds two float vectors. Track
mattes need only the local mask image unless their source uses other scratch.
Unused optional buffers have zero backing capacity. Allocated scratch survives
at the same extent; changing the extent drops all optional storage and resets
bindings, then allocates it again only if used. Transferring a cached image
replaces its storage, so caller-held outputs remain independently owned.

Scratch allocation/reuse counts are cumulative backend events; operation results
report their deltas. Scratch byte/buffer fields are current capacities, and cache
peaks are per-owner high-water marks. They cannot be added as if they were
simultaneous peaks. Per-frame matte-cache images and group-mask owner/group
temporaries are released before the idle snapshot; the worker estimate includes
their structural storage allowance. Container bookkeeping, prepared plan copies,
thread stacks, allocator-retained freed memory, audio preparation and encoder
processes are outside these pixel-storage gauges. Use external process sampling
for resident memory; see [performance](../performance.md).

## Resource estimates

`cpu_worker_memory_estimate_bytes` reports plan-based per-worker pixel storage.
It is independent of automatic worker selection and includes:

- One required scratch image and two SDK output payloads at output dimensions.
  Reachable compiled effect-pass topology adds one or two scratch images;
  masks/mattes add their optional images and vectors, with two feather vectors
  allowed for positive, animated or bound feathering. The estimate covers the
  retained union across timeline phases, including reachable hidden matte and
  group-mask sources, rather than only the features active at frame zero.
- Distinct retained composition pool depths reachable through visible groups
  and active matte chains, peak simultaneous matte-cache images, and maximum
  overlapping group-mask owner/group temporaries. Hidden sources are reached
  through active consumers; unused hidden and out-of-output layers add no storage.
- For native video, two source-sized RGBA images per independent cursor: current
  and scaler output, plus one incoming conversion at the largest reachable source
  size.
  CPU layer requests block until their frame is available, so incoming RGBA
  conversions do not overlap within a worker; native prefetch only decodes
  lookahead planes. Current frames remain allowed even with a zero cache budget;
  cached aliases do not justify removing the uncached allowance. Per-asset peak
  overlapping source clocks cover extra cursors; serial clips reuse the primary,
  which remains retained after the asset leaves view. Matching local origins
  and source start/rate share a cursor. Group and source-mask intervals use
  composition-local offsets and parent/output clipping. This conservatively
  ignores opacity, frame sampling and cache
  hits, and sums each asset's peak. Native row padding, YUV/lookahead and opaque
  codec state are excluded from the estimate. Custom decoder providers may
  retain additional internal storage that this native pixel model cannot bound.

All estimate arithmetic saturates on overflow and selection always permits at
least one worker. Shared decoded assets and configured cache budgets are
separate; the 512 MiB heuristic is not a total-memory guarantee. Static visual
plans still use one worker. Corrected accounting alone does not release memory;
any concurrency change needs paired throughput and process-memory measurements.

## Worker selection and memory limits

The selection budget is not an admission check. It does not query available
host memory, cgroup hard or soft limits, or process address-space limits. Each
CPU backend selects independently, without reserving future allocations across
prepared projects. Allowing at least one worker does not guarantee that an
encoded render fits in memory.

Whole-operation admission needs more information than the renderer pixel
estimate. Shared assets and audio analysis are prepared before backend
selection; the encoder process starts later. A prepared project can serve frame
requests without an encoder or execute a subsequent video operation. Those
operations have different resource requirements. Native codec state, thread
stacks and encoder buffers also depend on source formats, encoder settings and
the available hardware.

An available-memory snapshot cannot reserve future allocations for simultaneous
renders. OS memory limits can enforce isolation by terminating processes; they
do not provide graceful admission errors or guarantee output cleanup. Keep
resource accounting separate from selection policy. A future memory-aware
policy needs an operation-level resource model, ownership of outstanding
reservations, and a failure path when even the minimum operation cannot fit.

On Linux with cgroup v2, a headroom probe must consider host `MemAvailable` and
every visible cgroup ancestor's remaining `memory.max` and `memory.high`
allowances. The latter can trigger reclaim throttling before the hard limit is
reached. Process address-space/data limits constrain allocations separately from physical
memory; resident-memory readings cannot replace those checks. Other platforms
need their own limit discovery and an explicit policy for unavailable probes.

Preparation storage remains owned until the prepared project is dropped;
video-operation storage must be admitted before starting its encoder. A future
reservation scheme must distinguish already resident storage from outstanding
allocations to avoid double counting headroom. A process-local reservation can
coordinate that process's renders, but cannot reserve memory against unrelated
processes. External workload isolation remains a separate concern.
