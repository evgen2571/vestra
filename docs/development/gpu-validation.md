# GPU validation

WGPU execution is not hardware-GPU validation. A WGPU backend can select a software CPU adapter. Every hardware result must name the requested graphics API, actual graphics backend, adapter name and device classification reported by Vestra/WGPU.

1. Discover adapters:

   ```bash
   just wgpu-list
   ```

2. Read the `backend=`, adapter and `classification=` fields. A `discrete_gpu` or `integrated_gpu` classification is a hardware candidate. `cpu`/software classifications are not.
3. Choose the graphics API that exposes the hardware adapter and set it with `VESTRA_WGPU_BACKEND`, or let strict hardware verification select the first discovered hardware backend.
4. Run the appropriate mode and retain its discovery output with the test result.

```bash
just wgpu-list
VESTRA_WGPU_BACKEND=vulkan just wgpu-software
VESTRA_WGPU_BACKEND=gl just wgpu-hardware
```

Run these recipes inside the pinned development environment or an equivalent
native setup. They delegate to `scripts/verify-wgpu.sh` and preserve its adapter
selection and strict hardware requirements. CI uses `nix develop .#wgpu-software
--command just wgpu-software` to supply a software Vulkan environment.

`--software` selects the first discovered backend when none is requested and makes no hardware claim. It runs the general workspace suite without strict WGPU flags, then scopes `VESTRA_REQUIRE_WGPU=1` to renderer tests that need an adapter. `--hardware` rejects the selected backend unless discovery reports a `discrete_gpu` or `integrated_gpu`. It runs the general workspace suite without strict hardware assumptions, then scopes `VESTRA_REQUIRE_WGPU=1` and `VESTRA_REQUIRE_HARDWARE_WGPU=1` to adapter-dependent renderer tests and the strict CLI render regression. When no backend is supplied, `--hardware` selects the first discovered proven-hardware backend. It reports the selected discovery record before testing.

On some WSL installations, `vulkan` exposes llvmpipe while `gl`/GLES reaches NVIDIA through D3D12. That is an environment-specific result, not a universal rule. Start with `nvidia-smi`, `glxinfo -B` and `vulkaninfo --summary` when available, then trust adapter discovery. Set Vestra's `VESTRA_WGPU_BACKEND=gl` when the discovered GL path exposes the required hardware. Vestra does not read `WGPU_BACKEND` as a backend-selection variable.

llvmpipe and Lavapipe are useful software fallback/correctness adapters. They cannot support a hardware-performance or hardware-correctness claim. If discovery exposes only software adapters, report hardware validation as blocked. User selection details are in [Backends](../reference/backends.md).
