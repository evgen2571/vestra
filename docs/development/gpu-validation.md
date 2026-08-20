# GPU validation

WGPU execution is not hardware-GPU validation. A WGPU backend can select a software CPU adapter. Every hardware result must name the requested graphics API, actual graphics backend, adapter name and device classification reported by Vestra/WGPU.

1. Discover adapters:

   ```bash
   cargo run -p vestra-render --example wgpu_adapters --all-features
   ```

2. Read the `backend=`, adapter and `classification=` fields. A `discrete_gpu` or `integrated_gpu` classification is a hardware candidate. `cpu`/software classifications are not.
3. Choose the graphics API that exposes the hardware adapter and set it with `VESTRA_WGPU_BACKEND`.
4. Run the appropriate verification script and retain its discovery output with the test result.

`scripts/verify-wgpu.sh` requires `VESTRA_WGPU_BACKEND`, sets `VESTRA_REQUIRE_WGPU=1`, discovers adapters and runs software-correctness tests. It does not require hardware. `scripts/verify-wgpu-hardware.sh` first rejects the chosen API unless discovery reports a `discrete_gpu` or `integrated_gpu`; it then runs serialized workspace tests and strict adapter-dependent tests with `VESTRA_REQUIRE_HARDWARE_WGPU=1`.

On some WSL installations, `vulkan` exposes llvmpipe while `gl`/GLES reaches NVIDIA through D3D12. That is an environment-specific result, not a universal rule. Start with `nvidia-smi`, `glxinfo -B` and `vulkaninfo --summary` when available, then trust adapter discovery. Set Vestra's `VESTRA_WGPU_BACKEND=gl` when the discovered GL path exposes the required hardware. Vestra does not read `WGPU_BACKEND` as a backend-selection variable.

llvmpipe and Lavapipe are useful software fallback/correctness adapters. They cannot support a hardware-performance or hardware-correctness claim. If discovery exposes only software adapters, report hardware validation as blocked. User selection details are in [Backends](../reference/backends.md).
