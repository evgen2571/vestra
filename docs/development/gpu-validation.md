# GPU validation

WGPU selection is not hardware validation. Discover adapters first with `cargo run -p vestra-render --example wgpu_adapters --all-features`, identify a discrete or integrated adapter, select its graphics backend, then run validation and report requested backend, actual graphics backend, adapter name, and hardware/software classification.

In WSL, Vulkan can resolve to llvmpipe while GL/GLES resolves through D3D12 to NVIDIA hardware. Do not use llvmpipe as a hardware result. After discovery, `VESTRA_WGPU_BACKEND=gl scripts/verify-wgpu-hardware.sh` requires a classified hardware adapter. `scripts/verify-wgpu.sh` is software correctness validation only. Software adapters can test fallback correctness, never hardware performance.
