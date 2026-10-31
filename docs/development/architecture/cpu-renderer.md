# CPU renderer

The CPU backend in `vestra-render-cpu` implements the common `RenderBackend` contract for prepared resources and evaluated frame plans. It receives decoded image/video data and renderer-independent evaluated sources. It does not parse project JSON or decide which layers are active. Its job is to turn one evaluated frame into an owned RGBA frame for a caller or the SDK's media bridge.

Common preparation creates immutable image/shape/text pixels and glyph atlases shared by `Arc`. The CPU backend prepares worker-local raster views, video decoder sessions and source-specific resources before the hot loop. A frame then dispatches evaluated sources, including images, video frames, shapes, text, Spectrum2D, particles and recursively evaluated groups. Shared geometry and crop adaptation feed CPU sampling/compositing work; tiny-skia shape rasterization belongs to common preparation. Ordered layers and effect passes are composited into the output surface; nested compositions render their child-local work before their parent presentation is applied.

Source rasters skip bilinear interpolation when resolved geometry has a full
normalized source buffer, zero logical origin, source-sized raster extents,
unit inverse axes and exact integer translation. Clipping, colour transforms,
opacity and source-over composition still apply; zero-alpha source RGB is
normalized as in the filtered sampler. Fractional/near-identity transforms,
resizing, unresolved crop boundaries and Motion Tile retain filtering.
Materialized crop buffers can qualify when their resolved bounds satisfy the
same proof. This path allocates no additional storage.

The implementation keeps reusable surfaces, decoded data, worker-local state, bitmap/static-layer caches and source resources around the prepared render. Video decode remains a media-facing session but its pixels are consumed through prepared renderer resources. Reuse is important because a frame path that repeatedly allocates a canvas, opens a file, reparses a font, or rebuilds an effect state will distort both performance and renderer parity.

Under `crates/vestra-render-cpu/src/`, `cpu/compositor.rs` keeps recursive composition, masks/mattes and cache
presentation in one `ComposeContext`, which borrows the prepared assets and
surface pools. `cpu/effects.rs` keeps ordered pass execution with its convolution
implementation. Their pixel, ordering, cache and surface-reuse tests live in
`cpu/compositor/tests.rs` and `cpu/effects/tests.rs`, alongside test-only helpers.

`particle_raster.rs` and the pixel color transform in `blend.rs` belong to
`vestra-render-common`. WGPU's additive-particle compatibility path uses this same
rasterizer. CPU surface blending/counters, host CRT sampling and worker profiling
stay in the CPU implementation. CPU/WGPU comparison tests live in facade
integration modules and require both features; WGPU planning, packing and
resource-limit tests live in the WGPU package and run independently of CPU.

CPU capability validation belongs to `cpu/support.rs`. Its deliberate kernel
decisions derive from `cpu_effect_kernels!` in `cpu/effects.rs`, beside typed
operation dispatch. Every current kernel is implemented; a new common kernel
requires a registered or explicitly unsupported CPU row. Algorithms and surface
execution stay explicit. Common supplies only kernel identifiers, operation
mapping and required-kernel collection. The facade calls CPU/WGPU validation
before selecting or falling back to an implementation.

Opaque raster presentation has a small row-copy fast path. It is eligible only
for normal source-over at unit opacity with the full source crop, original
sizing, unit scale, zero rotation, integer translation, default colour
transform, no effects, masks or mattes, and every source pixel at alpha 255.
The clipped source and destination rows are copied directly; transparent,
filtered, scaled, transformed or otherwise uncertain layers use the existing
sampler and blend path. Layer opacity alone never proves source opacity.
Clipping rejects off-screen translations before integer narrowing, and byte
spans are checked before copying. Static source opacity is prepared once per worker;
video opacity currently scans the returned RGBA on each presentation, including
repeated frames and ineligible layers. Decoder providers do not supply a reusable
opacity proof. The focused [row-copy benchmarks](../../../benchmarks/README.md)
separate that scan cost from compositing and one-shot encoded rendering.
SDK copy-hit/byte and generic-blend counters describe the current operation;
reusing a prepared project does not include work from previous operations.

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
- Two source-sized RGBA images per independent native video cursor: current
  and scaler output, plus one incoming conversion at the largest reachable
  source size. CPU requests are serial within a worker; native prefetch decodes
  lookahead planes without converting RGBA. Retained current/scaler storage
  remains allowed with zero cache. Per-asset peak overlapping source clocks
  cover extra cursors; serial clips reuse the primary, retained after the asset
  leaves view. Matching local origins and source start/rate share a cursor.
  Group and source-mask intervals use composition-local offsets and parent/output
  clipping. Opacity, frame sampling and cache hits are conservatively ignored.

Estimate arithmetic saturates on overflow. Shared decoded assets and configured
cache budgets are separate. Native row padding, YUV/lookahead and opaque codec
state are excluded; custom decoder providers can retain additional internal
storage. This estimate is not a total-memory bound or a sum of resident bytes.
Accounting alone releases no memory.

## Worker selection and memory limits

Automatic selection deliberately preserves the original compatibility policy:
static visual plans use one worker; dynamic plans reserve one CPU when possible,
cap at eight, and divide 512 MiB by six output-sized RGBA frames per worker,
always permitting at least one. This legacy calculation depends only on canvas
size and CPU availability, not effects, sources, measured capacities or the
reported resource estimate. Correcting accounting or making scratch lazy must
not indirectly change concurrency. Performance and memory comparisons must use
matching worker counts; claim resident-memory savings only where demonstrated.

The legacy heuristic is not reliable whole-render admission. It does not query
host memory, cgroup pressure/hard limits or process allocation limits, and it
reserves nothing across prepared projects. Reducing concurrency to one cannot
guarantee that an encoded render fits. Existing OOM and simultaneous-render
limitations remain; neither accounting nor lazy scratch fixes them.

Whole-operation memory admission is deferred. Shared assets/audio analysis are
prepared before backend selection; an encoder starts later for a video operation,
while prepared-frame requests need no encoder. Native codec storage and thread
stacks depend on formats, encoder settings and hardware. An available-memory
snapshot cannot reserve future allocations, and OS-enforced kills do not provide
graceful admission errors or guaranteed output cleanup.

A future policy needs preparation/video costs, reservations owned for actual
resource lifetimes, and a structured failure when even the minimum operation
cannot fit. It must distinguish resident storage from outstanding allocations,
physical headroom from virtual/commit limits, and cgroup hard limits from reclaim
pressure. Process-local reservations cannot coordinate unrelated processes.
Platform discovery and isolation belong to that separate work.
