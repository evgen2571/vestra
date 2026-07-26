# WGPU renderer

The renderer consumes the same compiled plan, decoded assets, and
`EvaluatedFrame` as CPU:

```text
EvaluatedFrame → CPU compositor → RGBA → FFmpeg
               → GPU frame plan → texture frame graph → readback → RGBA → FFmpeg
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

Canonical raw-frame regression tests retain that two-channel maximum. The
separate MP4 decode comparison allows a maximum channel error of sixteen and a
mean error of one because H.264 quantization amplifies otherwise bounded raw
rounding differences; it is not used as compositor parity evidence.
Parity metrics retain the first tolerance-exceeding pixel index with both RGBA
values, so fixture failures can report compact context rather than frame contents.

## Resources and readback

WGPU owns persistent source textures, two canvas textures, a layer texture,
two effect slots, shaders, compute pipelines, a bounded dynamic-uniform buffer,
and a readback buffer. `Rgba8Unorm` working textures store encoded
straight-alpha channel values. This deliberately matches the CPU's byte-space
colour semantics. Shaders clamp each write. They do not perform linear-light
compositing or use sRGB storage textures.

Each frame has an adapter-independent plan:

```text
evaluated frame
  -> clear Canvas A
  -> render Layer texture
  -> future Effect A / Effect B chain
  -> composite Layer with Canvas A or B into the other canvas
  -> final canvas texture
  -> copy to readback buffer
```

Canvas A and B ping-pong, so no compute pass reads and writes the same texture.
The final slot is explicit in the plan, including empty, odd-layer, and
even-layer frames. The five working textures are allocated during preparation
and retained for the backend lifetime. Phase 1 accepts the bounded cost of five
full-output-size textures rather than adding variable-resolution pooling.

The frame parameter arena writes every operation record before command encoding.
Records are padded to `min_uniform_buffer_offset_alignment`, uploaded once, and
selected with dynamic offsets. Bind groups describe the operation's source and
destination views. Output rows are aligned to WGPU's copy-row requirement, then
repacked into a contiguous RGBA buffer before streaming to FFmpeg.

The compute shader samples with `textureLoad`, so source texture and byte
counters are reported separately and `sampler_count` is intentionally zero.

Every normal frame creates one command encoder and one queue submission. Clear,
layer work, ping-pong composition, and the final texture-to-readback copy all
live in that command buffer. Readback remains synchronous. There is no zero-copy
encoder path, hardware video encoding, windowed preview, or advanced GPU effects.

## Headless setup and diagnostics

On Linux install a Vulkan implementation, including a software ICD such as
Mesa Lavapipe for CI. Select discovery behavior with:

```bash
VIDEO_EDITOR_WGPU_BACKEND=vulkan
VIDEO_EDITOR_WGPU_FORCE_FALLBACK=1
VIDEO_EDITOR_REQUIRE_WGPU=1
```

The backend derives and validates output and source texture dimensions, padded
row/copy sizes, uniform size, texture bindings, and compute workgroup limits
before creating render resources. It
requests those project-derived limits on top of WGPU's downlevel baseline, then
checks the limits returned by the requested device again. Initialization, limit, and readback failures
are returned as structured diagnostics. Per-frame WGPU validation and internal
errors are captured with device error scopes, so they trigger the normal encoder
abort and output cleanup path. In environments without an adapter,
adapter-dependent parity tests print an explicit skip reason; this is not GPU
verification. With `VIDEO_EDITOR_REQUIRE_WGPU=1`, those tests fail instead of
skipping when adapter or device creation fails. The project-local
`nix develop .#software-vulkan` shell discovers Lavapipe through Nix's Mesa ICD
path and enables strict Vulkan verification.

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

The harness performs five warmups followed by five measured renders and prints
median and range values for total render and wall-clock time. Timing fields are
CPU-observed wall-clock values. Command encoding, queue submission, and
readback wait are not GPU execution timestamps.

`gpu_initialization_ms` is inclusive; its adapter request, device request, and
pipeline creation sub-stages are reported separately. Texture upload and all
per-frame stages are likewise CPU-observed durations, not hardware timestamps.

The texture frame graph replaced the old full-frame accumulation storage buffer.
Clear and layer dispatches no longer submit independently. The dynamic-uniform
arena preserves each operation's parameters until GPU completion, so all normal
work can be submitted together.

Phase 2 adds an effect by extending the shared `EffectPass`, adding a parameter
record, shader, pipeline dispatch, capability declaration, CPU/WGPU parity test,
and benchmark. Advanced effects and blend modes remain unsupported in WGPU until
that sequence is complete.
