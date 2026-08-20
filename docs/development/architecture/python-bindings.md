# Python binding architecture

The public Python package has two layers. The normal layer is pure Python and mutable: `Project`, `Composition`, `Layer`, sources, effects, transitions, audio and properties hold authoring intent. `Project.snapshot()` lowers that graph through `ProjectBuilder` and `LoweringContext` into an immutable native `ProjectSnapshot` (`vestra._native.Project`). Advanced authoring can use `ProjectBuilder` directly. Neither layer is a second renderer.

```text
high-level Python editing graph
  -> lowering / canonical ProjectBuilder JSON
  -> PyO3 ProjectSnapshot, Editor and requests
  -> public Rust SDK
  -> core + renderer + media
```

`vestra-python` is a thin PyO3 boundary over `vestra`. It converts paths, enums, JSON-like mappings, diagnostics, reports, frames, events and errors. `ProjectSnapshot` is immutable and supports loading/serialization. `Editor` owns semantic validation, preflight, inspection, preparation and one-shot render. `PreparedProject` keeps prepared native resources and exposes frame and video methods.

Native validation, preflight, preparation and rendering detach from the GIL. When a render has a `progress` callback, the binding reacquires Python only to invoke that callback and converts the native `RenderEvent`. Native observers can receive `started`, `progress`, and `completed`. Python callbacks receive only `started` and `progress`. The binding filters `completed` because native completion is emitted after output publication, so the callback cannot still cancel or change the finished render.

If a Python callback raises, the binding requests native cancellation, preserves that original Python exception, and re-raises it after cleanup. It never returns a `RenderResult` or replaces the callback exception with a generic `RenderError`. When cleanup reports a native error, the binding attaches its Python error object as `error.render_cleanup_error`. Ordinary cooperative cancellation is different: it raises `CancelledError`, a `RenderError`.

Native project/preparation/render errors become the documented Python exception hierarchy. Cooperative cancellation maps to `CancelledError`, a `RenderError`, rather than a result with partial success. Keep `.pyi`, PyO3 method signatures and package exports synchronized: users type against the stub, but behavior comes from the compiled extension.
