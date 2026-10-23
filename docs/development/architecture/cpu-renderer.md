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
