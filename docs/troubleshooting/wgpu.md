# WGPU problems

A WGPU selection can refer to a software adapter. Start with the reported
adapter and failure stage; changing the graphics API is useful only when you
know which API exposes the device you need.

## No adapter or the requested backend is unavailable

**Check.** Read the preparation/render diagnostic and adapter data. Separate
no adapter, an unavailable graphics API and device creation failure. If you
have a source checkout with the contributor environment, discover the visible
adapters with:

```bash
cargo run -p vestra-render --example wgpu_adapters --all-features
```

Installed-wheel users do not need a Rust build to collect the original
diagnostic or to try a CPU render.

**Fix.** Check the installed graphics driver and device access. If discovery
shows the intended adapter on a particular API, set `VESTRA_WGPU_BACKEND` to
that API. Accepted case-insensitive values are `vulkan`, `gl`/`gles`, `metal`,
`dx12` and `browser_webgpu`; unset or unrecognized values try all compiled
backends. This controls discovery, while `--render-backend wgpu` or
`BackendPreference.WGPU` requests Vestra's renderer.

**Confirm.** Prepare/render again and check the selected backend, adapter and
graphics API. Keep discovery output with the original diagnostic if device
creation still fails. See [backend selection](../reference/backends.md).

## WGPU selected the wrong adapter

**Check.** Read adapter name, graphics backend, driver and classification in the
preparation/render report. A CPU/software classification, llvmpipe or Lavapipe
indicates software WGPU. `OTHER` alone does not prove hardware.

**Fix.** Use a graphics API that actually exposes the intended hardware. On some
WSL systems Vulkan reaches llvmpipe while GL/GLES reaches NVIDIA through D3D12;
use `VESTRA_WGPU_BACKEND=gl` only after discovery confirms that configuration.
`VESTRA_WGPU_IN_FLIGHT` tunes pipeline depth, accepts only 1 through 3 and does
not select an adapter.

**Confirm.** Read adapter data from the new operation. Software-adapter success
is correctness evidence, not hardware-performance evidence. Follow
[GPU validation](../development/gpu-validation.md) for hardware measurements.

## ASCII differs from CPU on NVIDIA Vulkan

**Check.** Compare the adapter and driver with the known configuration: NVIDIA
GeForce GTX 1650 SUPER, Vulkan driver 595.71.05. Small-cell and animated ASCII
glyph comparisons can exceed CPU/WGPU parity tolerance there.

**Fix.** Use CPU for consistent ASCII output on this configuration. The same
cases pass with llvmpipe software Vulkan; that does not verify NVIDIA hardware.

**Confirm.** Render the same project/frame on CPU. If another configuration
shows the issue, include the two frames and adapter/driver data in
[a failure report](rendering.md#report-an-unresolved-render-failure).

## Fallback, in-flight work, and recovery

**Check.** Auto may fall back to CPU with `VESTRA-WGPU-FALLBACK`; an explicit
WGPU request can fail instead. Check requested versus selected backend and the
reported failed stage, rather than assuming the requested path ran.

**Fix.** Correct the driver/API/resource failure identified above. Remove an
invalid internal `VESTRA_WGPU_IN_FLIGHT` setting; use the default unless you are
following a specific profiling procedure. For resource and completion rules,
see [WGPU ownership](../development/architecture/wgpu-renderer.md#resource-ownership-and-reuse).

**Confirm.** Recreate prepared state and rerun. Verify the selected backend and
absence of the original warning or diagnostic. The
[environment reference](../reference/environment-variables.md) distinguishes
supported configuration from internal tuning.
