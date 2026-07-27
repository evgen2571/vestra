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
Effect A and Effect B textures, shaders, compute pipelines, a bounded
dynamic-uniform buffer, and a readback buffer. `Rgba8Unorm` working textures store encoded
straight-alpha channel values. This deliberately matches the CPU's byte-space
colour semantics. Shaders clamp each write. They do not perform linear-light
compositing or use sRGB storage textures.

Each frame has an adapter-independent plan:

```text
evaluated frame
  -> clear Canvas A
  -> render Layer texture
  -> ordered local passes through Effect A / Effect B
  -> composite affected Layer with Canvas A or B into the other canvas
  -> ordered global passes through Effect A / Effect B
  -> final canvas or effect texture
  -> copy to readback buffer
```

Canvas A and B ping-pong, and effect slots alternate, so no compute pass reads
and writes the same texture. Glow and sharpen retain the pre-effect input as an
auxiliary read while their composite pass writes the alternate effect texture.
The final slot is explicit in the plan, including empty, odd-layer, even-layer,
and global-effect frames. `ApplyEffect` carries local/global scope, optional
layer index, exact logical `EffectPass`, source, destination, auxiliary slot,
and pass index. The executor selects an already-prepared effect pipeline and
cached bind group from that operation; it never reinterprets an evaluated effect.

The frame parameter arena writes every operation record before command encoding.
`LayerParameters` is 176 bytes. Each bind group uses an explicit 176-byte
uniform binding range at buffer offset zero, and dynamic offsets select one
aligned record. The arena checks alignment, arithmetic, final-record bounds,
and the `u32` dynamic-offset conversion before upload. It prepares the maximum
frame capacity up front, so Phase 1 never grows the buffer. If a later backend
replaces that allocation, it must rebuild only bind groups that reference that
buffer.

Bind groups have clear ownership. Pipeline layouts and pipelines live for the
backend. Source-to-Layer groups live per uploaded source asset. Clear, solid
Layer, all usable Canvas-plus-Layer/Effect composite combinations, and all valid
source/auxiliary/destination effect combinations live for the fixed working
textures. No normal frame creates a bind group. Internal metrics count
groups created during preparation, capacity-growth rebuilds, per-frame groups,
cache hits, and cache misses.

The requirements calculation retains estimates for source textures, Canvas A
and B, Layer, Effect A and B, readback, and the parameter buffer. The total is
an estimate, not a driver VRAM measurement.
It excludes texture row padding, driver allocation overhead, mip levels,
implementation alignment, and temporary source staging allocations. Output rows
in the readback buffer do include WGPU's copy-row padding. Output rows are then
repacked into a contiguous RGBA buffer before streaming to FFmpeg.

The compute shader samples with `textureLoad`, so source texture and byte
counters are reported separately and `sampler_count` is intentionally zero.

Every normal frame creates one command encoder and one queue submission. Clear,
layer work, effect passes, ping-pong composition, and the final texture-to-readback copy all
live in that command buffer. Readback remains synchronous. There is no zero-copy
encoder path, hardware video encoding, windowed preview, or asynchronous readback.

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

Current WGPU effects use the same 176-byte dynamically bound record as layer
operations, with a typed operation discriminator and dedicated fields for
colour transforms, Gaussian direction/radius, sampling, spatial coordinates,
colour adjustment, and compositing controls. The effect shader implements the
current CPU catalogue: colour transforms, Gaussian blur, glow, sharpen,
directional and motion blur, zoom blur, chromatic aberration, vignette, and
colour adjustment. The blend shader implements normal, add, screen, multiply,
and overlay with the CPU's straight-alpha formulas and layer-opacity ordering.
Camera shake only changes evaluated geometry and therefore produces no pixel
pass. Presets and transitions are ordinary evaluated effects by this stage; no
WGPU-specific expansion exists.

To add a future effect, extend the shared `EffectPass`, its CPU executor,
WGPU parameter encoding, shader/pipeline mapping, frame-plan tests, capability
diagnostics, parity fixture, and benchmark. Do not add a backend-specific
interpretation of effect ordering.
