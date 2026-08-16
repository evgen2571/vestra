# Vestra

Rust video editing/rendering engine with core planning/evaluation, rendering
backends, FFmpeg I/O, SDK/CLI surfaces, and Python bindings.

## Architecture
- Preserve separation between project model, plan compilation/evaluation,
  renderers, media I/O, public APIs, CLI, and bindings.
- Core semantics should remain renderer-independent.
- CPU and GPU backends should implement the same supported semantics.
- FFmpeg/codec/process details belong behind the media-I/O boundary.
- CLI and Python bindings should orchestrate engine APIs, not duplicate engine logic.
- Use CodeGraph to confirm current crate/module ownership rather than assuming
  roadmap or historical structure is still current.

## Invariants
- Treat time/frame/sample conversions and boundary behavior as correctness-sensitive.
- Public Rust APIs, Python APIs, serialized formats, CLI contracts, and report
  schemas are compatibility-sensitive.
- If Rust API changes affect Python bindings, update and verify both.
- Avoid avoidable allocation/I/O in hot frame/render loops.

## GPU / WGPU policy
- Hardware WGPU and software WGPU are different validation classes.
- Never describe llvmpipe, Lavapipe, SwiftShader, software Vulkan, or another CPU
  adapter as GPU/hardware rendering.
- When GPU correctness or GPU performance is relevant, hardware validation is
  required unless the task explicitly says software WGPU is sufficient.
- Prefer a real discrete/integrated GPU adapter over CPU/software adapters.
- Do not silently fall back to CPU/software WGPU when hardware WGPU was requested.
- If hardware WGPU cannot be obtained, report the task as hardware-GPU blocked
  rather than treating a software-WGPU pass as sufficient.
- CPU fallback may be tested separately, but must never satisfy a hardware-WGPU
  requirement.

### WSL hardware-GPU preflight
When running inside WSL, validate graphics paths before WGPU tests:

```bash
nvidia-smi
glxinfo -B
vulkaninfo --summary
