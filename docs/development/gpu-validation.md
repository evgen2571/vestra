# GPU validation

WGPU execution is not hardware-GPU validation. A WGPU backend can select a software CPU adapter. Every hardware result must name the requested graphics API, actual graphics backend, adapter name and device classification reported by Vestra/WGPU.

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
software adapters are available, report hardware validation as blocked. A pass
on one graphics API does not establish another API's compatibility.

## Understand the test evidence

Both modes collect all private WGPU tests from
`vestra-render-wgpu --lib --features adapter-tests` and
cross-backend facade integration tests from `vestra-render --tests --all-features
gpu_`. For focused development, run the relevant package with the same strict
flags and selected API. Running only the facade library no longer collects the
GPU implementation or integration parity suites.

`just effect-check` instead disables the WGPU crate's default `adapter-tests`
feature. It checks registration, shader structure, parameter ABI/packing, pure
planning and resource calculations without initializing any graphics backend.
It does not replace either strict adapter suite.

## Troubleshoot platform discovery and execution

On some WSL installations, `vulkan` exposes llvmpipe while `gl`/GLES reaches NVIDIA through D3D12. That is an environment-specific result, not a universal rule. Start with `nvidia-smi`, `glxinfo -B` and `vulkaninfo --summary` when available, then trust adapter discovery. Set `VESTRA_WGPU_BACKEND=gl` when the discovered GL path exposes the required hardware.

GL rendering requires compute shaders and storage buffers; discovery of a desktop
GL 3.3 adapter alone does not establish a usable Vestra renderer. WGPU 23 also
creates a native indirect-call validator during device creation, so an
incompatible GL adapter can fail before Vestra creates its own compute pipelines.
The shared root-instance lifetime and GL context restrictions remain required.
Validate a compatible GL/GLES runtime on WSL separately from Vulkan; Vulkan
results do not establish GL/WSL, DX12, Metal or browser runtime compatibility.

llvmpipe and Lavapipe are useful software fallback/correctness adapters. They cannot support a hardware-performance or hardware-correctness claim. User selection details are in [Backends](../reference/backends.md).

Software and hardware verification use normal Rust test parallelism after the
WGPU 23 queue-identity correction. GL keeps one active context for WSL/Mesa
compatibility and exercises the same ownership checks serially. The explicit
concurrent lifecycle regression retains its own preparation/render/drop workers.

## Stress resource lifetimes

Run the longer lifecycle stress explicitly, with strict adapter requirements and
the appropriate graphics API/environment from above:

```bash
VESTRA_WGPU_BACKEND=vulkan VESTRA_REQUIRE_WGPU=1 \
  cargo test -p vestra --test public_sdk --all-features \
  public_wgpu_concurrent_lifecycle_stress -- --ignored --exact --nocapture
```

It requests 12 workers with 30 prepare/render/drop rounds each (GL uses one
worker for context ownership), retaining returned frames across subsequent preparations and worker teardown. Add
`VESTRA_REQUIRE_HARDWARE_WGPU=1` for hardware validation. The ordinary suite keeps
a smaller concurrent regression and the deterministic retained-device/dropped-queue
regression, including submission-index polling.

This stress checks repeated preparation, submissions, retained frames and teardown.
It does not measure sustained encoding, performance or all platform runtimes;
use [media verification](media-pipeline.md#verify-encoded-output) and
[performance](performance.md) for those questions.
