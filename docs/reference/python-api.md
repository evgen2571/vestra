# Python API

The normal Python entry point is the mutable high-level `vestra.Project`.
Advanced users can author the canonical model with
`vestra.authoring.ProjectBuilder`. Runtime control uses the immutable
`ProjectSnapshot` and `Editor` types.

## High-level authoring

`Project(size, fps, duration, ...)` owns `root`, an `AudioTimeline`, and output
policy. `Composition.add(source, start=0, duration=None, z=0, id=None, ...)`
creates a `Layer`. `Composition.group(...)` creates a `CompositionLayer` whose
`child` composition has child-local timing. Sources are exported from
`vestra.sources`, including `Image`, `Video`, `Color`/`SolidColor`, `Shape`,
`Text`, `Spectrum2D`, and `ParticleSystem` where the package exports them.

High-level projects provide `validate()`, `prepare(backend="auto")`,
`render(output, backend="auto", overwrite=False, preview=False, ...)`,
`render_frame(seconds)`, and `snapshot(output=...)`. `validate()` returns a
`ValidationReport` and does not inspect the environment. See [backends](backends.md).

## Advanced canonical authoring

`vestra.authoring.ProjectBuilder` owns canonical assets, clips, tracks,
effects, transitions, flashes, presets, and audio. Its `to_dict()` and
`to_json()` output the project format described in [Project format](project-format.md).
It is supported advanced authoring, not a deprecated API.

## Runtime and rendering

| Type | Contract |
| --- | --- |
| `ProjectSnapshot` | Immutable native project. Load with `load`, `from_json`, or `from_dict`; serialize with `to_json`, `to_dict`, and `save`. |
| `Editor` | `validate`, `preflight`, `inspect`, `prepare`, and one-shot `render`. |
| `PrepareOptions` | Optional `backend` preference. |
| `PreparedProject` | Reusable prepared runtime. Renders frames by number, nanoseconds, or seconds, and renders video with `PreparedVideoRenderRequest`. |
| `RenderRequest` | One-shot request for `Editor.render`: output, backend, overwrite, and preview. |
| `PreparedVideoRenderRequest` | Prepared render request: output and overwrite. |
| `RenderEvent` | Callback event with schema version, kind, frame, total frames, progress, output path, and warnings. |
| `RenderResult` | Successful publication summary with backend, adapter, timing, frame, audio, and output data. |
| `CancellationToken` | Cooperative cancellation flag. |

Prepared rendering must use `PreparedVideoRenderRequest`; `RenderRequest`
belongs to one-shot `Editor.render`. A cancelled render raises
`vestra.CancelledError`, a subclass of `RenderError`, rather than returning a
successful `RenderResult`.

## Reports and errors

`ValidationReport` has `is_valid`, `diagnostics`, `errors`, and `warnings`.
`PreflightReport` adds `is_ready`. Native failures include `ProjectError`,
`PreparationError`, `FrameRenderError`, `RenderError`, and
`PreparedProjectBusyError`. `Diagnostic` exposes `code`, `category`, `severity`,
`message`, `pointer`, `related_id`, and `hint`.

## Public enum values

`BackendPreference` is `AUTO`, `CPU`, or `WGPU`. `BackendKind` reports `CPU` or
`WGPU`; adapter metadata reports device type and graphics backend separately.
`RenderFailureStage` reports output preparation, asset preparation, encoder
startup, frame composition/write, encoder finalization, output publication, or
cancellation.

The runtime names above are exported by `vestra` and typed in
`python/vestra/_native.pyi`. The high-level names and canonical builder types
are defined in the Python package modules.
