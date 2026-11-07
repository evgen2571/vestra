# GPU validation

Choose the evidence you need before running a renderer check. WGPU can select a
software CPU adapter; successful WGPU execution alone does not demonstrate
hardware correctness. Frame inspection answers a further question: whether the
rendered result looks and behaves as intended.

| Evidence | What it establishes | What remains to check |
| --- | --- | --- |
| Adapter-free tests | Registration, shaders, parameter layout and planning agree without a device | Device execution and rendered pixels |
| Strict software-safe WGPU run | Tests execute on an available adapter; software adapters are permitted | Proven hardware behavior and visual quality |
| Strict hardware WGPU run | Tests execute on an integrated or discrete GPU for the selected graphics API | Other APIs/devices and visual quality |
| Rendered-output inspection | Selected frames/sequences match independent expectations | Untested scenes, parameters and devices |

Keep CPU, software-WGPU and hardware-WGPU results separate. Every hardware result
must name the requested graphics API, actual graphics backend, adapter name and
device classification reported by Vestra/WGPU.

Run from the repository root in the pinned development environment or an
equivalent native setup. The recipes delegate to `scripts/verify-wgpu.sh`.

## Discover and select an adapter

1. Discover adapters:

   ```bash
   just wgpu-list
   ```

2. Read the `backend=`, adapter and `classification=` fields. A `discrete_gpu` or `integrated_gpu` classification is a hardware candidate. `cpu`/software classifications are not.
3. Choose the graphics API that exposes the hardware adapter and set it with `VESTRA_WGPU_BACKEND`, or let strict hardware verification select the first discovered hardware backend.
4. Run the appropriate mode and retain its discovery output with the test result.

Discovery establishes adapter availability and classification, not successful
rendering. Vestra reads `VESTRA_WGPU_BACKEND`, not `WGPU_BACKEND`.

## Verify software-safe correctness

```bash
VESTRA_WGPU_BACKEND=vulkan just wgpu-software
```

This mode permits software adapters and makes no hardware claim. If no API is
requested, it selects the first discovered backend. It runs the workspace suite
without strict WGPU flags, then requires an adapter with `VESTRA_REQUIRE_WGPU=1`
for targeted renderer tests. CI supplies its software Vulkan environment with
`nix develop .#wgpu-software --command just wgpu-software`.

## Verify hardware correctness

```bash
VESTRA_WGPU_BACKEND=gl just wgpu-hardware
```

Choose the API from discovery; `gl` here is an example. This mode rejects a
backend unless discovery reports an integrated or discrete GPU. Without an API
request, it selects the first discovered proven-hardware backend and prints that
record. The workspace suite runs without strict hardware assumptions; targeted
renderer tests and the CLI render regression require both
`VESTRA_REQUIRE_WGPU=1` and `VESTRA_REQUIRE_HARDWARE_WGPU=1`.

Retain the discovery record and actual test adapter with results. If only
software adapters are available, report hardware validation as unverified and
state the missing hardware adapter. A pass on one graphics API does not establish
another API's compatibility.

## Understand the test evidence

Both strict modes run the general workspace suite without strict WGPU flags,
then require an adapter for these targeted suites:

- All private WGPU tests: `vestra-render-wgpu --lib --features adapter-tests`,
  including device submission, readback callbacks, resource and lifecycle tests.
- Cross-backend facade tests: `vestra-render --tests --all-features gpu_`,
  including rendering/parity checks.
- Hardware mode additionally runs the strict CLI encoded-frame render regression
  with hardware required.

For focused development, run the relevant suite with `VESTRA_REQUIRE_WGPU=1`,
the selected `VESTRA_WGPU_BACKEND`, and `VESTRA_REQUIRE_HARDWARE_WGPU=1` when
claiming hardware evidence. Running only the facade library does not collect
implementation-private tests or facade integration/parity suites.

`just effect-check` has a different purpose: it disables the WGPU crate's default
`adapter-tests` feature. Registration, shader parsing/composition, parameter
ABI/packing, pure planning and resource calculations still run, including tests
whose names contain `gpu_`. A test-only guard rejects graphics initialization
before discovery, so this command needs no Vulkan, EGL, software ICD or adapter.
Ordinary `cargo test -p vestra-render-wgpu` retains adapter tests by default.
Adapter-free tests do not replace either strict suite or output inspection.

## Inspect rendered output

A passing parity test can preserve the same mistake in both backends. Use
expected literal pixels, a known fixture/reference or a cache-disabled path as
an independent check, then inspect the actual frames:

1. Render a short deterministic sequence before, at and after effect/transition
   boundaries. Include nested offsets, masks/mattes and fractional FPS when the
   change affects them.
2. Repeat timestamps in forward/backward order and with fresh/prepared sessions
   to expose request-order state or cache invalidation.
3. Inspect transparent hidden RGB, alpha, semitransparent edges and composition
   over a visible backdrop. Fix seeds, fonts, assets and time.
4. Retain per-frame metrics and contact sheets; an average can hide a bad
   boundary frame. Justify tolerances for the operation and raw/encoded format,
   and investigate nondeterminism before changing thresholds.

Record the selected backend and actual adapter alongside the output. Keep
visual quality evidence alongside numerical comparisons. For encoded output,
also [probe the published media and check sync and cleanup](media-pipeline.md#verify-encoded-output).

## Troubleshoot platform discovery and execution

Start with `nvidia-smi`, `glxinfo -B` and `vulkaninfo --summary` when available,
then use Vestra's adapter discovery. On some WSL installations, Vulkan exposes
llvmpipe while GL/GLES reaches NVIDIA through D3D12; choose
`VESTRA_WGPU_BACKEND=gl` only when discovery confirms that hardware path.

GL rendering requires compute shaders and storage buffers; discovery of a desktop
GL 3.3 adapter alone does not establish a usable Vestra renderer. WGPU 23 also
creates a native indirect-call validator during device creation, so an
incompatible GL adapter can fail before Vestra creates its own compute pipelines.
The shared root-instance lifetime and GL context restrictions remain required.
Validate a compatible GL/GLES runtime on WSL separately from Vulkan; Vulkan
results do not establish GL/WSL, DX12, Metal or browser runtime compatibility.

llvmpipe and Lavapipe are useful for software correctness and fallback checks.
Use proven hardware for hardware correctness or performance claims. User
selection details are in [Backends](../reference/backends.md).

GL verification runs tests serially because WSL/Mesa permits one active context.
Other backends use normal Rust test parallelism. The explicit concurrent
lifecycle regression retains its own preparation/render/drop workers.

## Stress resource lifetimes

Run the longer lifecycle stress explicitly, with strict adapter requirements and
the appropriate graphics API/environment from above:

```bash
VESTRA_WGPU_BACKEND=vulkan VESTRA_REQUIRE_WGPU=1 \
  cargo test -p vestra --test public_sdk --all-features \
  public_wgpu_concurrent_lifecycle_stress -- --ignored --exact --nocapture
```

It requests 12 workers with 30 prepare/render/drop rounds each (GL uses one
worker for context ownership). It retains returned frames across subsequent
preparations and worker teardown. Add
`VESTRA_REQUIRE_HARDWARE_WGPU=1` for hardware validation. The ordinary suite keeps
a smaller concurrent regression and the deterministic retained-device/dropped-queue
regression, including submission-index polling.

This stress checks repeated preparation, submissions, retained frames and teardown.
It does not measure sustained encoding, performance or all platform runtimes;
use [media verification](media-pipeline.md#verify-encoded-output) and
[performance](performance.md) for those questions.
