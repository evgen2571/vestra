# WGPU renderer

The renderer consumes the same compiled plan, decoded assets, and
`EvaluatedFrame` as CPU:

```text
EvaluatedFrame → CPU compositor → RGBA → FFmpeg
               → WGPU compute compositor → output texture → readback → RGBA → FFmpeg
```

Backend selection is runtime-only:

```bash
video-editor render project.json --render-backend cpu --output cpu.mp4
video-editor render project.json --render-backend wgpu --output gpu.mp4
video-editor render project.json --render-backend auto --output auto.mp4
```

`cpu` never initializes WGPU. `wgpu` requires a compatible headless adapter
and never falls back. `auto` may fall back to CPU only while WGPU is being
prepared; a runtime GPU failure aborts the render and cleans up FFmpeg output.
Reports distinguish requested and selected backends, fallback context, adapter
metadata, and the FFmpeg encoder backend.

## Assets, coordinates, and crops

Images decode once into shared RGBA bytes. CPU owns its byte-budgeted crop
cache; WGPU uploads each full decoded source once and treats static cacheable
crops as virtual integer materializations using the CPU's floor/ceil crop
bounds. This makes crop sizing and transparent crop-edge sampling match the
CPU without creating a texture per frame.

Both renderers use a top-left origin, positive Y downward, destination pixel
centres, inverse affine transforms, manual bilinear sampling, and transparent
out-of-bounds samples. The compute compositor keeps straight RGBA values and
uses the CPU source-over equation with per-layer rounding. The initial parity
target is a maximum absolute channel error of two for floating-point image
cases; exact cases require exact bytes.

## Resources and readback

WGPU owns persistent source textures, shader, compute pipeline, bind groups,
accumulation buffer, output texture, and readback buffer. Output rows are
aligned to WGPU's copy-row requirement, then repacked into a contiguous RGBA
buffer before streaming to FFmpeg. Renderer-owned resource counters and GPU
preparation timings are exposed in reports.

Every frame still transfers back to CPU. There is no zero-copy encoder path,
hardware video encoding, windowed preview, or advanced GPU effects.

## Headless setup and diagnostics

On Linux install a Vulkan implementation, including a software ICD such as
Mesa Lavapipe for CI. Select discovery behavior with:

```bash
VIDEO_EDITOR_WGPU_BACKEND=vulkan
VIDEO_EDITOR_WGPU_FORCE_FALLBACK=1
```

The backend validates texture, buffer, storage-binding, and dispatch limits
before creating render resources. Initialization, limit, and readback failures
are returned as structured diagnostics. In environments without an adapter,
adapter-dependent parity tests print an explicit skip reason; this is not GPU
verification.

## Verification and benchmark

Run adapter-independent checks with:

```bash
cargo test --all-targets --all-features
./scripts/check.sh
```

On a compatible adapter, run WGPU parity tests and the release benchmark:

```bash
VIDEO_EDITOR_WGPU_BACKEND=vulkan cargo test --all-targets --all-features
VIDEO_EDITOR_BENCH_BACKEND=cpu cargo bench --bench animation_effects -- --nocapture
VIDEO_EDITOR_BENCH_BACKEND=wgpu cargo bench --bench animation_effects -- --nocapture
```

Benchmark timing fields are CPU-observed wall-clock values. Command encoding,
queue submission, and readback wait are not GPU execution timestamps.
