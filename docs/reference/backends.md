# Backends

Vestra has a renderer preference and, for WGPU, a graphics adapter. They are
different values.

| Preference | Meaning |
| --- | --- |
| `auto` | Let Vestra choose an available renderer according to the operation's backend policy. |
| `cpu` | Require the CPU renderer. |
| `wgpu` | Request the WGPU renderer. |

Results and preparation reports expose the requested preference, selected
backend, fallback information, and adapter metadata where available. The CLI
uses `--render-backend`; Python uses `BackendPreference` or a backend string.
`VESTRA_WGPU_BACKEND` selects the graphics API used for WGPU discovery, not
Vestra's renderer preference.

WGPU does not mean hardware. A WGPU adapter may be software, including a
CPU-class adapter. For example, WSL Vulkan may resolve to llvmpipe while the
GL path can resolve through D3D12 to NVIDIA hardware. Hardware claims require
the reported `AdapterDeviceType`, not merely `wgpu` selection or a Vulkan
backend name.

If WGPU cannot satisfy an automatic request, the operation may fall back to
CPU and report `VESTRA-WGPU-FALLBACK`. An explicit WGPU request follows the
current request/preflight policy and reports failure when fallback is not
allowed. Inspect `RenderResult`, `PreparationReport`, CLI result/report JSON,
or logs for actual selection data.
