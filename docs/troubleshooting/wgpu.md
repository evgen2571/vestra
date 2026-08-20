# WGPU problems

## No adapter or the requested backend is unavailable

Discover what WGPU can actually see before forcing a graphics API:

```bash
cargo run -p vestra-render --example wgpu_adapters --all-features
```

Set `VESTRA_WGPU_BACKEND` to the discovered WGPU graphics API. The accepted
case-insensitive values are `vulkan`, `gl` or `gles`, `metal`, `dx12`, and
`browser_webgpu`; an unset or unrecognized value lets wgpu try all compiled
backends. This chooses adapter discovery API. It does not choose Vestra's
renderer. Use `--render-backend wgpu` or `BackendPreference.WGPU` for that. A
missing adapter, an unavailable requested graphics API and a WGPU adapter that
later cannot create a device are separate failures; keep the discovery output
with the diagnostic.

## WGPU selected the wrong adapter

Read adapter name, graphics backend and classification in the preparation/render report. A `cpu`/software classification means WGPU is running, not that a hardware GPU was validated. llvmpipe and Lavapipe are valid software fallback/correctness paths. They are invalid evidence for hardware performance.

On some WSL systems Vulkan resolves to llvmpipe while GL/GLES reaches NVIDIA through D3D12. Verify each machine with adapter discovery; use `VESTRA_WGPU_BACKEND=gl` only when it exposes the desired hardware. See [GPU validation](../development/gpu-validation.md) for the required hardware procedure.

## Fallback, in-flight work, and recovery

With an automatic renderer preference, WGPU unavailability may result in CPU fallback plus `VESTRA-WGPU-FALLBACK`. An explicit WGPU request follows preflight policy and can fail instead. `VESTRA_WGPU_IN_FLIGHT` is internal tuning, accepts only 1 through 3 and should not be set to solve adapter selection. Use the public [backend contract](../reference/backends.md) and [environment variables](../reference/environment-variables.md) for scope.
