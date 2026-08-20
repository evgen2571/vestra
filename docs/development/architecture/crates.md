# Crate ownership

The workspace has six crates. Dependencies point toward the canonical model, with `vestra` as the only public orchestration layer used by the CLI and Python extension.

```text
vestra-cli ──> vestra <── vestra-python
                  │  ├──> vestra-core
                  │  ├──> vestra-render ──> vestra-core
                  │  └──> vestra-media ──> vestra-render (contracts), vestra-core
```

| Crate | Owns | May depend on |
| --- | --- | --- |
| `vestra-core` | Canonical project structures, semantic validation, timeline/plan compilation and evaluation, descriptors. | Serialization and small data utilities only. |
| `vestra-render` | `RenderBackend` contracts, CPU and WGPU backends, decoded rendering resources, compositing, text and effects. | `vestra-core`. |
| `vestra-media` | FFmpeg-native probing/decoding, audio analysis and graph execution, frame sinks, encoder finalization and output publication. | `vestra-core`, renderer contracts with rendering features disabled. |
| `vestra` | Public SDK project wrapper, validation/preflight, preparation, render orchestration, results, events and errors. | Core, render and media. |
| `vestra-python` | PyO3 classes and conversion between Python and the Rust SDK. | `vestra`. |
| `vestra-cli` | Clap parsing, logging subscriber, terminal/JSON progress and report presentation. | `vestra`. |

This direction is an ownership rule, not merely an import preference. Core cannot acquire a WGPU device, read a file, or invoke FFmpeg. Renderer code consumes compiled/evaluated semantics instead of applying JSON defaults or timeline rules again. Media code owns process/native-media details, including the gap between a finalized temporary file and an atomically published destination. CLI and bindings call `Editor`; they do not recreate validation or rendering flow.

When a change seems to need an import in the opposite direction, first look for a value or trait that the inner layer can own. For example, media depends on renderer frame contracts without depending on a concrete CPU or WGPU implementation.
