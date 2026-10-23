# WGPU renderer

The WGPU backend is a `RenderBackend` implementation that keeps GPU state inside one prepared render. It creates a WGPU instance, selects an adapter under the requested graphics API, requests the device and queue, then builds pipelines, textures, buffers, source resources and readback state. `AdapterInfo` records the actual graphics backend and device classification so the SDK can distinguish a hardware adapter from software WGPU.

Preparation uploads static image/font/source data and creates reusable GPU resources. Frame evaluation still happens outside the backend. For each evaluated frame, the backend uploads changed video data, builds source/effect/composite passes from the evaluated layer tree, renders nested groups recursively, submits commands and schedules a readback of the completed RGBA frame. The readback path accounts for row alignment and repacks rows into the public tightly packed frame format.

The backend has bounded in-flight work. `VESTRA_WGPU_IN_FLIGHT` controls the implementation pipeline depth only when valid; it is not a semantic setting. Texture pools, source caches, staging buffers, pending static layers and readback slots are retained across a prepared operation. Release them only when their GPU completion token makes them safe to reuse. This is why a `PreparedProject` owns one backend instance and is not freely concurrent.

The internal frame path has these owners, all inside
`crates/vestra-render/src/wgpu/`:

| Module | Owns |
| --- | --- |
| `backend.rs` | Prepared resources and per-submission slots, video uploads, cache reservations, polling, cancellation and lifecycle coordination |
| `frame_plan/mod.rs`, `frame_plan/operations.rs` | Frame-plan interface, authoritative operation and texture-role definitions, canvas alternation |
| `frame_plan/builder.rs` | Ordered frame construction, recursive layers/groups, static-cache operations and compositing |
| `frame_plan/masks.rs` | Mask coverage, feathering, isolated sources and matte planning |
| `frame_plan/effects.rs` | Core effect-pass expansion, logical-resource liveness and physical texture-slot binding |
| `frame_plan/validation.rs` | Texture initialization and logical values, aliasing, operation sequencing and diagnostics |
| `parameters/` | Typed WGSL-compatible records, packing and the aligned uniform arena; `parameters/frame.rs` encodes records in operation order and prepares particle ranges/uploads |
| `bind_groups.rs` | Prepared bind-group creation, reuse and lookup, plus binding construction for cached layer textures |
| `executor.rs` | Parameter upload, command encoding, pipeline dispatch, texture copies and submission metrics |

The private `FramePlanBuilder` owns the operation/layer vectors and shared
parameter/value counters. Recursive calls retain explicit canvas and logical
value transitions. Effect expansion and validation consume the same operation
and texture definitions; this remains a fixed-slot planner.

Bind groups remain owned by the backend's frame slots. Most are prepared once;
bindings for static cached textures are still created on demand during command
encoding. Keeping that timing preserves the existing allocation and metric
scopes. Topology, requirements, pipelines, texture pools and readback retain
their separate resource responsibilities.

Metrics distinguish initialization, command encoding/submission, uploads, readback waits and row repacking. A request for WGPU can still fail at adapter, device, pipeline, upload or runtime-pass boundaries. Automatic renderer selection may report `VESTRA-WGPU-FALLBACK` and use CPU; an explicit request follows preflight policy. Do not hide either case behind a generic "GPU" label.

This page describes implementation ownership. For adapter commands and hardware classification, use [GPU validation](../gpu-validation.md). For user recovery steps, use [WGPU troubleshooting](../../troubleshooting/wgpu.md). The public selection contract is in [Backends](../../reference/backends.md).
