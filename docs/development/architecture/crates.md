# Crate ownership

Choose a crate by the rule or resource you need to change. The workspace has
eleven crates; Python authoring code lives separately under `python/vestra/`.

| Crate | Change it for |
| --- | --- |
| `vestra-core` | Canonical project data, validation, timing, compilation/evaluation and effect descriptors. |
| `vestra-render-common` | Shared backend contracts, completed frames, prepared visual assets, geometry and numerical utilities. |
| `vestra-render-cpu` | CPU frame execution, compositing, effects, workers and backend caches. |
| `vestra-render-wgpu` | GPU frame execution, shaders, pipelines, device resources and readback. |
| `vestra-render` | Backend selection/fallback, renderer exports and cross-backend integration tests. |
| `vestra-media` | Probing/decoding, audio execution, frame sinks, encoding and output publication. |
| `vestra` | Public Rust SDK, preflight, preparation, render orchestration, results and errors. |
| `vestra-progress` | Typed render lifecycle events, progress policy and shared terminal presentation. |
| `vestra-observability` | Application tracing setup, filtering, human/JSON logs and coordinated stderr/file output. |
| `vestra-python` | PyO3 conversion of Python requests, frames, reports, events and errors to/from the SDK. |
| `vestra-cli` | `ve` argument parsing, SDK requests and command-line result presentation. |

## Dependency direction

In this compact view, **A → B means A directly depends on B** in its production
Cargo dependencies. It does not describe runtime order; test-only dependencies
are omitted. All names below are workspace crates.

| A | → B |
| --- | --- |
| `vestra-cli`, `vestra-python` | `vestra`, `vestra-observability` |
| `vestra` | `vestra-core`, `vestra-render`, `vestra-media`, `vestra-progress` |
| `vestra-render` | `vestra-core`, `vestra-render-common`, optional CPU/WGPU implementations |
| `vestra-render-cpu`, `vestra-render-wgpu` | `vestra-core`, `vestra-render-common` |
| `vestra-render-common`, `vestra-media`, `vestra-progress` | `vestra-core` |
| `vestra-observability` | `vestra-progress` |
| `vestra-core` | No workspace crates |

The renderer facade's `cpu` and `wgpu` features select its implementations;
both are enabled by default. Frontends configure logging through observability
and use the shared progress policy rather than implementing their own lifecycle.

## Cross a boundary with data

Core cannot acquire a device, read an asset or invoke FFmpeg. Renderers consume
compiled/evaluated semantics rather than applying project defaults or timeline
rules again. CPU and WGPU implementations do not depend on each other or on
the facade.

Renderers and media also remain independent. Media's
`FrameSink::write_frame(frame_number, rgba)` accepts a number and borrowed bytes.
The SDK retains the renderer's `CompletedFrame` until the write finishes,
including any scoped writer. A change that seems to require a reverse dependency
usually belongs in this coordinating layer or in the data passed across the
boundary.

Follow the [render pipeline](render-pipeline.md) for frame lifetime and output
completion, or [project to plan](project-to-plan.md) for core's validation and
evaluation boundaries.
