# Crate ownership

The workspace has eleven crates. Dependencies point toward the canonical model,
with `vestra` as the public orchestration layer used by the CLI and Python
extension. `vestra-observability` owns application-boundary subscriber setup;
`vestra-progress` owns the frontend-neutral lifecycle and native terminal
presentation.

```text
vestra-cli ──> vestra <── vestra-python
                  ├──> vestra-core
                  ├──> vestra-progress ──> vestra-core
                  ├──> vestra-media ──> vestra-core
                  └──> vestra-render ──> vestra-core
                           ├──> vestra-render-common ──> vestra-core
                           ├──> vestra-render-cpu ──> vestra-render-common, vestra-core
                           └──> vestra-render-wgpu ──> vestra-render-common, vestra-core
```

CLI and Python also depend on `vestra-observability`, which uses
`vestra-progress`. Renderer implementation dependencies are selected by the
facade's `cpu` and `wgpu` features; both are enabled by default.

| Crate | Owns | May depend on |
| --- | --- | --- |
| `vestra-core` | Canonical project structures, semantic validation, timeline/plan compilation and evaluation, descriptors. | Serialization and small data utilities only. |
| `vestra-render-common` | `RenderBackend` lifecycle, completed frames, shared immutable prepared assets, geometry, kernel identifiers/requirements and shared numerical utilities. | `vestra-core`. |
| `vestra-render-cpu` | CPU execution, workers, compositing, effects, mutable caches, profiling and CPU capability validation. | Common and core. |
| `vestra-render-wgpu` | GPU execution, shaders, pipelines, resources, readback and WGPU capability validation. | Common and core. |
| `vestra-render` | Public renderer facade, selection/fallback, compatibility reexports and cross-backend integration tests. | Common, core and feature-selected CPU/WGPU implementations. |
| `vestra-media` | FFmpeg-native probing/decoding, audio analysis and graph execution, frame sinks, encoder finalization and output publication. | `vestra-core`; no renderer crate. |
| `vestra` | Public SDK project wrapper, validation/preflight, preparation, render orchestration, results, events and errors. | Core, render and media. |
| `vestra-progress` | Typed RenderEvent v1 lifecycle, progress modes, and shared native terminal presentation. | `vestra-core`. |
| `vestra-observability` | Application-boundary tracing subscriber, EnvFilter/RUST_LOG policy, human/JSON formatting, and stderr/file output coordinated with progress. | `vestra-progress`. |
| `vestra-python` | PyO3 classes and conversion between Python and the Rust SDK, including explicit observability configuration. | `vestra`, `vestra-observability`. |
| `vestra-cli` | Clap parsing and report/result presentation; adapts CLI verbosity/progress choices to shared crates. | `vestra`, `vestra-observability`, `vestra-progress`. |

This direction is an ownership rule, not merely an import preference. Core cannot acquire a WGPU device, read a file, or invoke FFmpeg. Renderer code consumes compiled/evaluated semantics instead of applying JSON defaults or timeline rules again. Media code owns process/native-media details, including the gap between a finalized temporary file and an atomically published destination. CLI and bindings call `Editor`; they do not recreate validation or rendering flow.

When a change seems to need an import in the opposite direction, first look for
the data the inner layer consumes. Media's `FrameSink::write_frame` accepts a
frame number and borrowed RGBA bytes. The SDK bridges the renderer's owned
`CompletedFrame` to that sink, retaining ordering, cancellation and frame lifetime.
Media and renderers do not import one another, and CPU/WGPU implementations do
not depend on their sibling or the facade, including in development dependencies.

Common stores shared prepared pixels behind `Arc` and exposes borrowed resource
accessors. Backend-specific execution and capability policy stay in the respective
implementation. Hidden public bridges exist for sibling workspace crates;
`doc(hidden)` controls documentation, not Rust visibility. Gaussian float weights
are retained in every configuration with the same bounded thread-local cache;
there is no feature-dependent kernel layout.
