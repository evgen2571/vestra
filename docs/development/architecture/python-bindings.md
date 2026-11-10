# Python binding architecture

The public Python package separates mutable authoring from native execution.
`Project`, `Composition`, `Layer`, sources, effects, transitions, audio and
properties hold authoring intent in pure Python.

`Project.snapshot()` lowers that graph through `ProjectBuilder` and
`LoweringContext` into an immutable native `ProjectSnapshot`
(`vestra._native.Project`). Advanced callers can use `ProjectBuilder` directly.
Both authoring routes use the Rust SDK for rendering.

```text
high-level Python editing graph
  -> lowering / canonical ProjectBuilder JSON
  -> PyO3 ProjectSnapshot, Editor and requests
  -> public Rust SDK
  -> core + renderer + media
```

## Native operations and prepared state

`vestra-python` converts Python paths, enums, mappings and requests into SDK
operations, then returns reports, frames, events or errors through PyO3.
`ProjectSnapshot` supports immutable loading and serialization. `Editor`
provides validation, preflight, inspection, preparation and one-shot rendering;
`PreparedProject` retains native resources for repeated frame and video requests.

Python resolves FFmpeg lazily in `vestra._ffmpeg` using the automatically
installed `imageio-ffmpeg` dependency or `IMAGEIO_FFMPEG_EXE`. The binding passes
an absolute path through an SDK editor clone; preparation retains it for master
analysis and every raw/static video operation. No environment mutation or
process-wide native configuration is used. Import, semantic validation and media
inspection continue to use only the native libraries. Rust/CLI editors retain
their default PATH lookup unless explicitly configured with
`Editor::with_ffmpeg_executable`.

## Progress, logging and callback failures

Tracing remains native and renderer-independent. Python does not configure a
second logger or duplicate lifecycle records; native render spans and
`RenderEvent` values share one operation identifier. Python callbacks receive
the typed lifecycle and may opt out of the built-in terminal presentation with
`show_progress=False`.

Native validation, preflight, preparation and rendering detach from the GIL. A
Python `on_progress` callback reacquires Python only to invoke that callback and converts the native
RenderEvent v1. Callbacks receive `started`, `stage_changed`, `progress`, and
the terminal outcome. The callback replaces the native Auto terminal renderer;
without a callback, ordinary rendering still uses the shared native progress
policy. `Completed` is emitted only after output publication.

Python logging is explicit: `vestra.configure_logging(...)` maps to the shared
`vestra-observability` subscriber and does not run during import or ordinary
rendering. The binding exposes ergonomic level, format, output, file, and
EnvFilter settings without duplicating Rust formatting or filtering.

If a Python callback raises, the binding requests native cancellation and
re-raises the original Python exception after cleanup. It does not return a
`RenderResult`. A cleanup error is attached as `error.render_cleanup_error`,
preserving the callback exception.

Native failures map to the documented Python exception hierarchy. Cooperative
cancellation raises `CancelledError`, a `RenderError`. Keep `.pyi`, PyO3 method
signatures and package exports synchronized: type checking uses the stub;
execution uses the compiled extension.

## Source lowering and font ownership

High-level Text resolves its optional bundled font or installed family at the
authoring/lowering boundary, retaining canonical Font asset IDs and file paths.
Private PyO3 discovery uses fontdb, scans once per process, and returns an exact
case-insensitive family's preferred regular face as a file/index pair. Python
materializes collection faces as content-addressed standalone sfnt files in the
user cache; this keeps the renderer's file-backed contract and avoids rendering
a different collection face. Explicit font files retain existing base-directory
resolution and preparation diagnostics. No canonical schema fields are added.

Standalone `Group` sources lower through the source registry into the same
canonical group clips as `CompositionLayer`. Child placements use ordinary
source handlers, scoped deterministic IDs and the snapshot's shared asset
registry. Source transform defaults are copied into placement transforms once;
individual overrides replace only the selected property's animation/bindings.

For a binding change, keep the native signature, `.pyi` and package exports
in agreement and use [Testing](../testing.md) for focused Python checks. Public
user-facing methods are listed in [Python API](../../reference/python-api.md).
