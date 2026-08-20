# Crate ownership

The workspace has six crates: `vestra-core`, `vestra-render`, `vestra-media`, `vestra`, `vestra-python`, and `vestra-cli`. Dependency direction is inward to `vestra-core` for semantics, then outward through renderer/media implementation and the public SDK. CLI and Python call the public SDK instead of rebuilding validation or rendering logic.

Keep canonical model and compilation renderer-independent. Renderers consume compiled/evaluated semantics. Keep FFmpeg, probing, process execution, and publication behind `vestra-media`.
