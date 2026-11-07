# CPU renderer

The CPU backend in `vestra-render-cpu` implements the common `RenderBackend` contract for prepared resources and evaluated frame plans. It receives decoded image/video data and renderer-independent evaluated sources. It does not parse project JSON or decide which layers are active. Its job is to turn one evaluated frame into an owned RGBA frame for a caller or the SDK's media bridge.

Use this page to locate CPU work and interpret resource reports. For the SDK
frame loop, see [render pipeline](render-pipeline.md); for measurements, see
[performance](../performance.md).

CPU worker selection is a canvas-size heuristic, **not a whole-render memory
limit**. See [worker selection and memory limits](#worker-selection-and-memory-limits)
before interpreting the resource reports below.

## Preparation

Common preparation creates immutable image/shape/text pixels and glyph atlases shared by `Arc`. The CPU backend prepares worker-local raster views, video decoder sessions and source-specific resources before the hot loop.

## Frame execution

A frame then dispatches evaluated sources, including images, video frames, shapes, text, Spectrum2D, particles and recursively evaluated groups. Shared geometry and crop adaptation feed CPU sampling/compositing work; tiny-skia shape rasterization belongs to common preparation. Ordered layers and effect passes are composited into the output surface; nested compositions render their child-local work before their parent presentation is applied.

## Ownership and caching

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

Preserve these boundaries when changing CPU code:

- Source and effect dispatch must consume evaluated values and respect plan ordering.
- Every returned frame is tightly owned RGBA8 data. Callers must not borrow a mutable CPU surface after the backend advances.
- Group recursion must retain child-local timing and layer ordering.
- Caches may change cost, never visible semantics. Key them with all inputs that affect a cached result.
- Keep I/O, project validation and public-path resolution out of the per-frame loop.

The CPU backend is the direct fallback when WGPU is unavailable under an automatic preference. It is not a reduced semantic mode; a documented limitation needs explicit source/effect/backend evidence.

## Fast paths and operation metrics

Source rasters skip bilinear interpolation when resolved geometry has a full
normalized source buffer, zero logical origin, source-sized raster extents,
unit inverse axes and exact integer translation. Clipping, colour transforms,
opacity and source-over composition still apply; zero-alpha source RGB is
normalized as in the filtered sampler. Fractional/near-identity transforms,
resizing, unresolved crop boundaries and Motion Tile retain filtering.
Materialized crop buffers can qualify when their resolved bounds satisfy the
same proof. This path allocates no additional storage.

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
opacity proof. The focused [row-copy benchmarks](https://github.com/evgen2571/vestra/blob/develop/benchmarks/README.md)
separate that scan cost from compositing and one-shot encoded rendering.
SDK copy-hit/byte and generic-blend counters describe the current operation;
reusing a prepared project does not include work from previous operations.

### Sparse convolution

The floating-point Gaussian path precomputes byte-to-unit normalization and
restricts work to
nonzero-alpha bounds expanded by finite kernel support. Sampling and clamping
stay in original canvas coordinates; reused output is cleared when work covers
only part of the canvas. A nontransparent corner bypasses the bounds scan, and
fully transparent input can avoid convolution. The fixed-point input-detail
path still convolves the full canvas.

Sparse glow/bloom can benefit; scanning an almost full-frame bound can add cost
without saving much work. Scratch dimensions remain unchanged. Isolated
group/matte rendering clears shared scratch after child composition before
group presentation so child effects cannot contaminate group coverage.

## Worker selection and memory limits

<span id="current-concurrency-policy"></span>

Automatic selection in `cpu/backend.rs` uses this policy:

- Static visual plans use one worker.
- Dynamic plans reserve one CPU when possible and cap at eight workers.
- A 512 MiB allowance divided by six RGBA canvases further limits the count,
  always permitting at least one worker. Canvas bytes use the larger of the
  output area and the largest explicit visual or owned-mask group canvas.

This calculation uses canvas sizes and CPU availability. It does not use effect
topology, media sizes, measured capacities or the reported worker estimate.
Compare performance and memory with matching worker counts.

<span id="current-limits"></span>

The heuristic does not query host memory, cgroup pressure/hard limits or process
allocation limits, and reserves nothing across prepared projects. Even one
worker can exceed available memory; OOM and simultaneous-render risks remain.
Accounting and lazy scratch allocation provide no admission guarantee.

<span id="deferred-admission-policy"></span>

[Whole-operation memory admission (#81)](https://github.com/evgen2571/vestra/issues/81)
owns the deferred design and acceptance criteria: preparation/video costs,
resource-lifetime reservations, concurrent operations, platform limits and a
structured cannot-fit error. Those requirements are not current behavior.

## Memory accounting

Resource reports describe named engine storage, not process RSS/PSS or a
whole-process limit. CPU snapshots are taken after worker work has completed.

### Shared payloads and budgeted caches

| Metric | Counted storage |
| --- | --- |
| `decoded_source_bytes` | Logical image, shape/text and glyph-atlas pixels, counted once across workers. |
| `cache_current_bytes` | Crop-cache logical pixel bytes charged to its budget. |
| `static_cached_bytes` | Whole-layer logical pixel bytes charged to its budget. |

These fields exclude spare backing capacity. Shared payloads also exclude
preparation temporaries and font-system internals. Cache budgets are separate
from scratch capacity and native video storage.

### Retained backing capacity

| Metric | Counted storage |
| --- | --- |
| `cpu_scratch_buffers_retained`, `cpu_scratch_bytes_retained` | Allocated RGBA images and `f32` vectors per worker, including returned nested-composition and matte-save pool surfaces. |
| `video_cache_bytes_retained` | Native video RGBA cache capacity. |
| `video_current_frame_bytes_retained` | Native current-frame capacity only when its pixels are absent from the cache; `Arc` aliases add no capacity. |
| `video_decoder_buffer_bytes_retained` | Stride-inclusive RGBA scaler and queued lookahead plane spans. |

One scratch image is always retained. Effect destinations and optional mask
storage allocate on first use. Masks need local/transformed images and coverage;
positive feathering adds two float vectors. Track mattes need only the local
mask image unless their source uses other scratch. Sibling compositions reuse
pool depths. Scratch survives at the same extent; resizing drops optional
storage and resets bindings before later use allocates it again. Transferring a
cached image replaces its storage so caller-held outputs remain independently owned.

Video gauges exclude evicted frames still held by callers, buffer tail padding,
codec reference/reorder frames, packets, demux/scaler internals and other opaque
native allocations. Custom decoder providers that do not measure memory leave
these gauges zero.

### Counters, peaks and exclusions

| Field | Interpretation |
| --- | --- |
| Scratch allocation/reuse counts | Cumulative backend events; operation results report deltas. |
| `cpu_full_frame_allocations` | Cumulative output-buffer allocations, not live output bytes. |
| `cpu_output_frame_bytes_estimated` | Two tight RGBA output payloads per worker in the SDK scheduler. |
| `video_cache_bytes_peak` | Sum of individual session high-water marks, including retired sessions. |

Per-owner cache peaks are not simultaneous peaks. Do not add them to current
capacity gauges as if all storage coexisted. Arbitrary caller-held frames after
transfer and the SDK's static final-frame template are outside backend gauges.
Per-frame matte-cache images and group-mask owner/group temporaries are released
before the idle snapshot; the worker estimate below includes their structural
allowance.

Container bookkeeping, prepared plan copies, thread stacks, allocator-retained
freed memory, audio preparation and encoder processes are outside these
pixel-storage gauges. Use [external process sampling](../performance-probes.md#allocation-and-sustained-render-profiling)
for resident memory.

## Resource estimates

`cpu_worker_memory_estimate_bytes` is a plan-based per-worker pixel-storage
estimate from `cpu/memory.rs`, independent of automatic worker selection.
Canvas allowances use the larger of output and largest explicit group canvas
bytes; native video allowances use source dimensions.

| Component | Allowance |
| --- | --- |
| Base canvases | One required scratch image and two SDK output payloads. |
| Effect passes | One or two additional images according to reachable compiled pass topology. |
| Masks/mattes | Optional images and vectors; two feather vectors for positive, animated or bound feathering. |
| Composition storage | Distinct retained pool depths, peak simultaneous matte-cache images, and maximum overlapping group-mask owner/group temporaries. |
| Native video | Two source-sized RGBA images per independent cursor (current and scaler output), plus one incoming conversion at the largest reachable source size. Serial worker requests and prefetch lookahead avoid simultaneous incoming RGBA conversions. |

The estimate covers the retained union across timeline phases, including hidden
matte and group-mask sources reached by active consumers. Unused hidden and
out-of-output layers add no storage. Group/source-mask intervals use
composition-local offsets and parent/output clipping.

Per-asset peak overlapping source clocks determine extra video cursors. Matching
local origins and source start/rate share a cursor; serial clips reuse the
primary cursor, retained after the asset leaves view. Current/scaler storage is
allowed even with zero cache. Opacity, frame sampling and cache hits are
conservatively ignored.

Arithmetic saturates on overflow. Shared decoded assets and configured cache
budgets are separate. Native row padding, YUV/lookahead and opaque codec state
are excluded; custom decoder providers may retain additional internal storage.
This estimate is neither a total-memory bound nor a sum of resident bytes.
