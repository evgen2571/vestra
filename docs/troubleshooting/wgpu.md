# WGPU problems

Use adapter discovery before forcing WGPU: `cargo run -p vestra-render --example wgpu_adapters --all-features`. `VESTRA_WGPU_BACKEND` selects a graphics API for discovery, not Vestra's CPU/WGPU preference. Set renderer preference with `--render-backend wgpu` or the SDK backend option.

No adapter, a requested unavailable backend, and a software-only adapter are different problems. In WSL, Vulkan may expose llvmpipe while GL/GLES exposes D3D12/NVIDIA hardware. Use GL after discovery in that case. See [GPU validation](../development/gpu-validation.md); llvmpipe is not a hardware result.
