# Backends

Every render operation goes through one of two renderers: **CPU** or **WGPU**
(the GPU path built on [wgpu](https://wgpu.rs), which selects Vulkan, Metal,
Direct3D 12 or GL at runtime). Both consume the same compiled project plan;
authoring does not change with the backend.

## Choosing a backend

Three values stay distinct:

1. The **requested** backend is what the caller asked for.
2. The **selected** backend is what Vestra actually prepared.
3. The **adapter** is the device behind a WGPU selection — which may be
   software (for example llvmpipe), not a hardware GPU.

| Preference | Meaning |
| --- | --- |
| `auto` (default) | Use WGPU when a usable adapter is available, otherwise CPU. If WGPU preparation fails, fall back to CPU and report `VESTRA-WGPU-FALLBACK`. |
| `cpu` | Require the CPU renderer. Predictable for a first render and for CI; needs no graphics driver. |
| `wgpu` | Require the WGPU renderer; the operation fails if no usable adapter is found. |

Python passes `"auto"`/`"cpu"`/`"wgpu"` or `BackendPreference` to `render()`,
`prepare()` and `render_frame()`. The CLI takes the same values as
`--render-backend`. The result reports what ran:

```python
result = project.render("out.mp4", backend="auto")
print(result.selected_backend.value)    # "cpu" or "wgpu"
print(result.adapter)                    # adapter name/type/backend, or None
print(result.fallback)                   # BackendFallback when WGPU was skipped
```

`VESTRA_WGPU_BACKEND` restricts which graphics APIs wgpu considers (for
example `vulkan` or `gl`); it does not select Vestra's renderer preference.
`WGPU_BACKEND` is not read. See [environment variables](environment-variables.md).

## What "hardware" means

`wgpu` does not imply a hardware GPU. A WGPU adapter can itself be a CPU
implementation. To claim GPU rendering, check the reported adapter:
`AdapterInfo.device_type` distinguishes discrete, integrated, virtual and CPU
adapters through `AdapterDeviceType` values. `CPU` identifies a CPU adapter;
`OTHER` alone does not establish whether it is hardware or software. Inspect
`adapter_name`, `graphics_backend`, `driver_name` and `driver_info` as well.
For example, llvmpipe and Lavapipe are software implementations. Performance
claims should name the adapter, graphics API and driver. See the
[native declarations](https://github.com/evgen2571/vestra/blob/develop/python/vestra/_native.pyi) for Python `AdapterInfo` fields.

## Parity and coverage

CPU and WGPU implement the same project semantics, but support is not uniform:
Spectrum2D and particle systems are not fully verified on WGPU, Motion Tile
needs a hardware adapter, and per-effect differences remain documented rather
than claimed. The [feature matrix](feature-support.md) is the authoritative
per-feature support record.
