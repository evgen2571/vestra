# WGPU renderer

The renderer consumes the same compiled plan, decoded assets, and
`EvaluatedFrame` as CPU:

```text
EvaluatedFrame → CPU compositor → RGBA → FFmpeg
               → GPU frame plan → texture frame graph → readback → RGBA → FFmpeg
```

Backend selection is runtime-only:

```bash
vestra render project.json --render-backend cpu --output cpu.mp4
vestra render project.json --render-backend wgpu --output gpu.mp4
vestra render project.json --render-backend auto --output auto.mp4
```

`cpu` never initializes WGPU. `wgpu` requires a compatible headless adapter
and never falls back. `auto` may fall back to CPU only while WGPU is being
prepared; a runtime GPU failure aborts the render and cleans up FFmpeg output.
Reports distinguish requested and selected backends, fallback context, adapter
metadata, and the FFmpeg encoder backend.

The current WGPU implementation has plan mappings for every valid
schema-version 3 effect, blend mode, post-effect, generic transition, and
preset.
Compatibility is separate from availability. A compatible plan can still fail
to obtain an adapter or device, prepare resources, compile a shader, or run.
`auto` falls back only during preparation. Explicit WGPU and selected WGPU
operations report failures instead of switching to CPU.

## Assets, coordinates, and crops

Images decode once into shared RGBA bytes. CPU owns its byte-budgeted crop
cache; WGPU uploads each full decoded source once and treats static cacheable
crops as virtual integer materializations using the CPU's floor/ceil crop
bounds. This makes crop sizing and transparent crop-edge sampling match the
CPU without creating a texture per frame.

Video uses one compact persistent raster slot per compiled Video layer. Decoder
sessions and initial frames are created only for assets referenced by those
slots; the initial selected PTS is retained so the first render does not upload
the same frame again. Later uploads occur only when a layer selects a new PTS.

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
Effect A and Effect B textures, and an Auxiliary texture only for projects
that contain glow or sharpen, plus shaders, compute pipelines, and a bounded
readback ring. Each in-flight slot owns its dynamic-uniform buffer, bind groups,
readback buffer, callback state, and packed output storage. `Rgba8Unorm` working textures store encoded
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
and writes the same texture. The layer shader always writes raw layer pixels:
basic colour effects now execute only as ordered `ApplyColourTransform` passes,
before layer opacity is applied during composition. This prevents the former
direct-colour path from applying the same logical transform twice.

Glow and sharpen copy their pre-effect logical value to Auxiliary before their
working passes start. Their final composite reads that retained value while
writing the alternate effect texture. The frame-plan validator tracks each
texture's initialized state and monotonically assigned logical value. Every
effect pass declares its expected source and fresh result value; composition
declares expected layer and canvas values plus its fresh canvas result; readback
declares the final expected value. The validator rejects stale retained reads,
stale local/global effect inputs, invalid overwrites, source/destination
aliasing, stale composition reads, and readback of anything other than the
final value. Auxiliary is reused only after the preceding effect's final
composite consumes it.
The final slot is explicit in the plan, including empty, odd-layer, even-layer,
and global-effect frames. `ApplyEffect` carries local/global scope, optional
layer index, exact logical `EffectPass`, source, destination, auxiliary slot,
and pass index. The executor selects an already-prepared effect pipeline and
cached bind group from that operation; it never reinterprets an evaluated effect.

Each frame-slot parameter arena writes every operation record before command encoding.
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
Effect A is allocated only when the compiled plan has a visual pass; Effect B
is allocated only when it has more than one possible pass and can therefore
need ping-pong. Transform-only projects reserve neither effect texture.
Auxiliary is allocated only when a compiled local or global glow/sharpen may be
active. These are allocation estimates, not exact VRAM measurements.
It excludes texture row padding, driver allocation overhead, mip levels,
implementation alignment, and temporary source staging allocations. Output rows
in the readback buffer do include WGPU's copy-row padding. Output rows are then
repacked into a contiguous owned RGBA buffer before streaming to FFmpeg.

The compute shader samples with `textureLoad`, so source texture and byte
counters are reported separately and `sampler_count` is intentionally zero.

Every normal frame creates one command encoder and one queue submission. Clear,
layer work, effect passes, ping-pong composition, and the final texture-to-readback copy all
live in that command buffer. Submission returns after `map_async` is initiated.
The polling subsystem first processes callback results, returns any ready frame,
makes nonblocking progress, and only then waits. `WaitForOne` waits for the
oldest relevant submission rather than draining later work; `Drain` completes
all active slots. Cancellation-aware polling uses bounded nonblocking progress
so a device wait cannot delay cleanup indefinitely. There is no zero-copy
encoder path, hardware video encoding, or windowed preview.

## Staged WGPU lifecycle

The engine uses one lifecycle for CPU and WGPU video and single-frame operations:

```text
evaluate → submit → in flight → map callback → repack → completed
         → ordered write → FFmpeg → slot reuse
```

The default WGPU depth is three. Tests and benchmarks select one, two, or three
slots with `VESTRA_WGPU_IN_FLIGHT`. A slot is reusable only after GPU copy,
mapping, row repacking, unmapping, completion consumption, and generation advance.
The callback captures a submission token containing frame number, slot index,
generation, and queue submission identity. A stale token cannot complete a
newer frame. The adapter-independent readback state machine owns legal slot
transitions, rejects duplicates and stale generations, and restores every slot
on abort.

Completion callbacks only publish a result. They do not copy rows or perform
diagnostic work. The render thread polls the device in one WGPU polling module,
copies tightly packed rows into owned storage, unmaps the buffer, and hands an
owned `CompletedFrame` to the engine. The engine keeps a bounded ordered queue and
writes only the next frame number. Out-of-order completion therefore cannot
reorder FFmpeg input.

Every row-layout calculation uses checked arithmetic. Invalid strides, host-size
conversions, source/destination lengths, and row offsets produce a structured
`WGPU-READBACK-SIZE` backend diagnostic. The production repacker is tested
directly for padded and unpadded layouts; padding bytes never appear in public
`Frame` pixels.

`Editor::prepare` retains this WGPU backend for both `render_frame_number` and
`render_frame`; neither method creates an encoder, output file, device, pipeline,
or texture upload. A returned SDK `Frame` is always top-row-first, tightly packed,
straight-alpha RGBA8 in independent CPU-owned storage. A readback callback carries
the frame, slot, and monotonically checked slot generation. A stale or duplicate
callback is a backend-contract failure: the prepared project is invalidated rather
than risking completion delivery to a later operation. After a successful frame
operation, `flush` and `verify_idle` require no in-flight work, mapped slot, queued
completion, or unavailable readback slot.

Shared Canvas, Layer, and effect textures remain backend-owned. WGPU preserves
submission order on one queue. Submission N copies its final texture into its
dedicated readback buffer before submission N+1 can reuse the working textures.
Parameter buffers are per-slot, so a later submission cannot overwrite an earlier
frame's dynamic records or destroy a bind group still referenced by work.

The engine submits only while slot capacity and ordered-ready capacity permit it.
Progress events and public completion counts still mean frames accepted by FFmpeg.
Internal metrics distinguish evaluated, submitted, backend-completed, ready, and
written frames. Accumulated stage work can overlap and can therefore sum to more
than wall-clock render time.

Cancellation stops evaluation and submission, aborts FFmpeg, discards ready frames,
invalidates slots, and removes the temporary output. WGPU work already submitted
to the device cannot be cancelled. Uncaptured WGPU errors and device loss retain
the first fatal diagnostic; submit, poll, and flush check that state and never
switch to CPU.

The SDK preserves an originating renderer diagnostic unchanged, including its
backend category, code, severity, pointer, hint, and related identifier. SDK-only
lifecycle failures use distinct `MVP-*` diagnostics. Device loss invalidates the
prepared backend; there is no automatic recovery or backend re-preparation.

`VESTRA_WGPU_IN_FLIGHT=1 cargo test --workspace --all-features` exercises the
synchronous-compatible depth. Strict verification and the optional benchmark matrix
run with:

```bash
VESTRA_WGPU_BACKEND=vulkan scripts/verify-wgpu.sh
VESTRA_WGPU_BACKEND=gl VESTRA_RUN_BENCHMARKS=1 \
  scripts/verify-wgpu-hardware.sh
```

The available Vulkan adapter may be Lavapipe/llvmpipe software rendering.
Software WGPU is useful for smoke, resource-lifecycle, validation, and readback
checks, but its timings do not demonstrate GPU acceleration and it is not a
hardware parity result. The hardware verification script enables
`VESTRA_REQUIRE_HARDWARE_WGPU=1`, so hardware-conformance parity fails rather
than being reported as passed on a software adapter.

## Headless setup and diagnostics

On Linux install a Vulkan implementation, including a software ICD such as
Mesa Lavapipe for CI. Select discovery behavior with:

```bash
VESTRA_WGPU_BACKEND=vulkan
VESTRA_WGPU_FORCE_FALLBACK=1
VESTRA_REQUIRE_WGPU=1
# Require a non-software adapter for hardware-conformance parity tests.
VESTRA_REQUIRE_HARDWARE_WGPU=1
```

The backend derives and validates output and source texture dimensions, padded
row/copy sizes, uniform size, texture bindings, and compute workgroup limits
before creating render resources. It
resolves those project-derived requirements against the discovered adapter and
requests only Vestra's required limits on top of WGPU's minimum baseline, then
checks the limits returned by the requested device again. Initialization, limit,
and readback failures are returned as structured diagnostics. Normal staged
submission does not synchronously await per-frame error scopes because that
would serialize the pipeline; asynchronous uncaptured and device-loss callbacks
trigger the normal encoder-abort and output-cleanup path. In normal mode, adapter-dependent hardware-conformance parity tests print
`WGPU_RUNTIME_SKIPPED reason=software-adapter` for Lavapipe/llvmpipe and similar
software adapters. This is distinct from the software-WGPU smoke tests, which
continue to execute. With `VESTRA_REQUIRE_HARDWARE_WGPU=1`, that software
adapter decision becomes a failure; with `VESTRA_REQUIRE_WGPU=1`, no compatible
adapter is also a failure. Device creation and every later backend failure
always fail. The project-local
`nix develop .#software-vulkan` shell discovers Lavapipe through Nix's Mesa ICD
path and enables strict Vulkan verification.

Adapter-gated tests run with `-- --nocapture` and emit
`WGPU_RUNTIME_EXECUTED adapter=<name> backend=wgpu` when their runtime body
executes, or `WGPU_RUNTIME_SKIPPED reason=no-compatible-adapter ...` when normal
mode encounters an exclusively adapter-unavailable diagnostic. Cargo records an
early return as passed, so the marker—not the pass count—distinguishes a real
runtime execution from an environment skip. Strict mode converts that same
adapter absence into a failure; device, shader, pipeline, texture, project, and
all mixed failures always fail in either mode.

## Verification and benchmark

For software-WGPU smoke validation, require an adapter but allow a software
adapter and do not set `VESTRA_REQUIRE_HARDWARE_WGPU`:

```bash
VESTRA_WGPU_BACKEND=vulkan \
  VESTRA_REQUIRE_WGPU=1 cargo test -p vestra-render --lib --all-features -- \
  --nocapture --test-threads=1
```

For hardware-WGPU conformance, use a real GPU and enable the explicit hardware
requirement. The hardware script sets both strict variables itself:

```bash
VESTRA_WGPU_BACKEND=vulkan scripts/verify-wgpu.sh
VESTRA_WGPU_BACKEND=gl scripts/verify-wgpu-hardware.sh
```

Run adapter-independent checks with:

```bash
cargo test --all-targets --all-features
./scripts/check.sh
```

On a compatible adapter, run WGPU parity tests and the release benchmark:

```bash
VESTRA_WGPU_BACKEND=vulkan cargo test --all-targets --all-features
VESTRA_BENCH_BACKEND=cpu cargo bench -p vestra --bench animation_effects -- --nocapture
VESTRA_BENCH_BACKEND=wgpu cargo bench -p vestra --bench animation_effects -- --nocapture
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

Gaussian-derived passes use one backend-neutral representation:
`(clamp(radius, 0, 32) * 4).round() / 4`. Gaussian blur, glow, and sharpen use
that canonical value to determine identity and encode Gaussian parameters.
Directional, zoom, and motion blur preserve their raw evaluated sampling
radius and are identity only at `radius <= 0.01`; they are never quarter-step
quantized. The same family-specific predicates drive evaluated pass planning
and CPU execution, while WGPU receives the canonical Gaussian or raw sampling
value appropriate to its pass.

Run the strict real-GPU verification sequence with:

```bash
VESTRA_WGPU_BACKEND=gl ./scripts/verify-wgpu-hardware.sh
VESTRA_WGPU_BACKEND=gl VESTRA_RUN_BENCHMARKS=1 \
  ./scripts/verify-wgpu-hardware.sh
```

The script fails if no compatible adapter is available or an adapter-dependent
test skips. It runs the effect catalogue, blend matrix, generated-effect
fixtures, chain/timeline coverage, and representative explicit-WGPU renders;
its optional benchmark matrix covers 320x180, 720x1280, and 1920x1080. Render
JSON and benchmark output include adapter name, backend, device type, driver,
classification, and software status. Software adapters validate correctness
only. Real-hardware CPU/WGPU performance benchmarking was deferred because this
environment has no suitable GPU.

The public example asset is `examples/assets/red.png`, currently a 160x90
1-bit indexed-colour PNG. Its exact bytes and PNG header are protected by
`./scripts/verify-public-asset.sh`; WGPU-specific image fixtures remain under
`tests/assets`.

To add a future effect, extend the shared `EffectPass`, its CPU executor,
WGPU parameter encoding, shader/pipeline mapping, frame-plan tests, capability
diagnostics, parity fixture, and benchmark. Do not add a backend-specific
interpretation of effect ordering.
