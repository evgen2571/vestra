# WGPU renderer

The WGPU backend is a `RenderBackend` implementation that keeps GPU state inside one prepared render. It creates a WGPU instance, selects an adapter under the requested graphics API, requests the device and queue, then builds pipelines, textures, buffers, source resources and readback state. `AdapterInfo` records the actual graphics backend and device classification so the SDK can distinguish a hardware adapter from software WGPU.

Preparation uploads static image/font/source data and creates reusable GPU resources. Frame evaluation still happens outside the backend. For each evaluated frame, the backend uploads changed video data, builds source/effect/composite passes from the evaluated layer tree, renders nested groups recursively, submits commands and schedules a readback of the completed RGBA frame. The readback path accounts for row alignment and repacks rows into the public tightly packed frame format.

The backend has bounded in-flight work. `VESTRA_WGPU_IN_FLIGHT` controls the implementation pipeline depth only when valid; it is not a semantic setting. Texture pools, source caches, staging buffers, pending static layers and readback slots are retained across a prepared operation. Release them only when their GPU completion token makes them safe to reuse. This is why a `PreparedProject` owns one backend instance and is not freely concurrent.

Metrics distinguish initialization, command encoding/submission, uploads, readback waits and row repacking. A request for WGPU can still fail at adapter, device, pipeline, upload or runtime-pass boundaries. Automatic renderer selection may report `VESTRA-WGPU-FALLBACK` and use CPU; an explicit request follows preflight policy. Do not hide either case behind a generic "GPU" label.

This page describes implementation ownership. For adapter commands and hardware classification, use [GPU validation](../gpu-validation.md). For user recovery steps, use [WGPU troubleshooting](../../troubleshooting/wgpu.md). The public selection contract is in [Backends](../../reference/backends.md).
