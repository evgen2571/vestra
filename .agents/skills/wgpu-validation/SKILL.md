---
name: wgpu-validation
description: "Use when verifying Vestra WGPU shader execution, adapter selection, CPU/GPU parity, texture or buffer limits, resource lifetimes, staging or readback regressions."
---

# Validate WGPU on the actual adapter

Read [GPU validation](../../../docs/development/gpu-validation.md)
and [WGPU architecture](../../../docs/development/architecture/wgpu-renderer.md).
Use Vestra's established `scripts/verify-wgpu.sh` and `just` recipes instead
of building a competing adapter validation framework.

## Workflow

1. Discover adapters with `just wgpu-list`. Record `backend=`,
   adapter name and `classification=`. Verify the actual selected
   backend using Vestra's output, not vendor utilities alone.
2. Select a graphics API if needed via `VESTRA_WGPU_BACKEND`
   (for example `vulkan`, `gl`, `metal` or `dx12` where supported).
3. Separate **software correctness** (`just wgpu-software`)
   from **hardware verification** (`just wgpu-hardware`).
   The strict hardware recipe requires an integrated or discrete adapter.
4. While iterating, run filtered `cargo test -p vestra-render --lib
   --all-features` and shader/parameter/resource tests. Follow
   `scripts/verify-wgpu.sh` for adapter-required test flags and GL test
   serialization; don't force hardware requirements onto general tests.
5. Investigate shader parse and binding errors, WGSL/Rust parameter
   alignment, pass ordering, texture-slot aliasing, limits, staging,
   readback and GPU completion lifetimes. Add focused negative tests
   and actionable diagnostics.
   Trace acquire → submit → map/complete → copy/unmap → reuse for each slot;
   keep parameters, uploads and pending cache reservations tied to their
   submission token/generation. Exercise multiple in-flight frames, eviction,
   abort and failed mapping; CPU submission is not GPU completion.
   Check padded-row sizes, host conversions and buffer growth against the
   requested device limits, not just adapter maxima. Returned frames must
   own pixels after readback slots are reused.
6. Use [visual-regression](../visual-regression/SKILL.md) to compare
   **actual rendered** CPU/WGPU frames when fidelity matters.

## Report

Give requested graphics API, selected adapter, classification, commands,
pass/fail results and remaining unverified environments. Never describe
llvmpipe/Lavapipe or any software adapter as GPU-hardware validation.
If hardware is unavailable, report hardware validation **unverified** with
the adapter/environment limitation; shader parsing and software results do
not establish hardware correctness or speed.
