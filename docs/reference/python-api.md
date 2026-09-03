# Python API

`vestra.Project` is the normal mutable authoring API. `vestra.authoring.ProjectBuilder` is the advanced canonical authoring API. Runtime control uses immutable native `ProjectSnapshot` (`vestra._native.Project`) and `Editor`.

## High-level authoring

```python
Project(*, size: tuple[int, int], fps: int | tuple[int, int] | FrameRate,
        duration: float | None = None, background="#000000", quality=Quality.BALANCED,
        base_directory=".", name=None, metadata=None, output_audio=None, output_path=None)

composition.add(source, *, start=0, duration=None, source_start=0.0,
                playback_rate=1.0, z=0, visible=True, opacity=1.0,
                id=None, name=None, blend_mode=BlendMode.NORMAL) -> Layer
composition.group(name=None, *, start=0, duration=None, z=0, visible=True,
                  opacity=1, id=None, blend_mode=BlendMode.NORMAL) -> CompositionLayer
```

`size` has positive integer width/height; `fps` is an integer, `(numerator, denominator)` pair or `FrameRate`; explicit `duration` is positive seconds. `root` is the top-level `Composition`; a `CompositionLayer.child` is another composition with child-local timing. `add` copies its `Source`; `duration=None` uses the owning duration except for `Video`, where it probes media to derive the usable source duration. `source_start` is source-media seconds and `playback_rate` is positive.

```python
project.snapshot(*, output=None) -> ProjectSnapshot
project.validate() -> ValidationReport
project.prepare(*, backend="auto") -> PreparedProject
project.render_frame(seconds, *, backend="auto") -> Frame
project.render(output, *, backend="auto", overwrite=False, preview=False,
               progress=None, show_progress=True, on_progress=None,
               cancellation=None) -> RenderResult
```

`project.render("output.mp4")` uses native Auto progress by default. Set
`show_progress=False` to disable it, or pass `on_progress=my_callback` to
replace the built-in terminal renderer with a callback. The older
`progress=` spelling remains a compatibility alias and is not the preferred
name.

`validate()` lowers then performs canonical semantic validation only. It does not probe files or create a renderer. `snapshot()` defaults output to the construction-time output path or `"output.mp4"`; `output_audio=None` follows whether authored audio clips exist. See individual [source pages](sources/image.md) for source constructors.

Layers expose an ordered mask collection:

```python
mask = layer.masks.add(shape, operation=MaskOperation.INTERSECT, id=None, feather=12)
layer.masks.items
layer.masks.remove(mask_or_id)
layer.masks.clear()
```

`Mask` exposes `id`, `input`, `operation`, `invert`, `strength`, `feather`, and
`transform`. Supported inputs are the existing `Rectangle`, `Ellipse`,
`Circle`, and `Polygon` shape authoring objects, or the normal `Image` source
with `mode=ImageMaskMode.ALPHA` or `ImageMaskMode.LUMA`. `strength`, `feather`, and
transform properties support the normal keyframes, modifiers, and signal
bindings. Masks are applied after layer effects and before final
opacity/blending.

`ProjectBuilder(*, width, height, frame_rate, output_path, duration=None, duration_mode=None, background="#000000", quality=Quality.BALANCED, base_directory=".", name=None, metadata=None, output_audio=False)` owns advanced canonical assets, visual clips, tracks, effects, transitions, flashes and audio. `build()` creates a native snapshot; `validate()` has the same semantic-only meaning.

## Native runtime

| Type | Exact construction or methods |
| --- | --- |
| `ProjectSnapshot` | `load(path)`, `from_json(text, *, base_directory=None)`, `from_dict(data, *, base_directory=None)`, `to_json()`, `to_dict()`, `save(path)`. |
| `Editor` | `Editor()`, then `validate(project)`, `preflight(project, options)`, `inspect(project, *, preview=False)`, `prepare(project, options=None)`, `render(project, request, *, on_progress=None, progress=None, cancellation=None)`. `progress` is a compatibility alias. |
| `PrepareOptions` | `PrepareOptions(*, backend=None)`. |
| `PreparedProject` | `render_frame_number(frame_number)`, `render_frame_ns(timestamp_ns)`, `render_frame_seconds(seconds)`, `render_video(request, *, on_progress=None, progress=None, show_progress=True, cancellation=None)`. `progress` is a compatibility alias. |
| `RenderRequest` | `RenderRequest(output, *, backend=None, overwrite=False, preview=False)` for one-shot `Editor.render`. |
| `PreparedVideoRenderRequest` | `PreparedVideoRenderRequest(output, *, overwrite=False)` for `PreparedProject.render_video`. |
| `CancellationToken` | `CancellationToken()`, `cancel()`, read-only `is_cancelled`. |

`RenderEvent` v2 exposes `schema_version`, `kind`, `operation_id`, `stage`,
`frame`, `total_frames`, `fraction`, and `output_path` with the applicable
fields optional. Its lifecycle is `started`, `stage_changed`, `progress`, and
one terminal event: `completed`, `cancelled`, or `failed`. Pass either
`progress` or `on_progress`, not both. Set `show_progress=False` to disable
the built-in Auto terminal presentation. Custom callbacks replace the built-in
terminal renderer. `Completed` occurs only after encoder finalization and
successful output publication. Rendering may reach fraction `1.0` before
encoding and finalizing complete. A callback error before terminal success may
abort the operation; an error while receiving `Completed` cannot invalidate a
successfully published render. A successful `RenderResult` exposes output
dimensions/timing, selected/requested backend, fallback, adapter, warnings,
detailed `timings` and `performance`. `CancelledError` is a `RenderError`; a
cancelled operation does not return a `RenderResult`.

If a Python progress callback raises, Vestra stops the native render and
re-raises the original Python exception. No `RenderResult` is returned. If
cleanup reports a native error, it is attached as
`error.render_cleanup_error`; it does not replace the callback exception.

## Logging

Logging is explicit and uses the shared Rust observability implementation;
importing Vestra and ordinary rendering do not install a global subscriber:

```python
import vestra

vestra.configure_logging(level="info")
vestra.configure_logging(level="debug", format="json", file="vestra.log")
```

`level` accepts `error`, `warn`, `info`, `debug`, and `trace`. `format` is
`human` or `json`. Output is `stderr` by default, or `file`/`stderr_and_file`
when `file` is supplied. `filter=` accepts an EnvFilter directive such as
`"vestra=info,vestra.render.wgpu=debug"`; `RUST_LOG` remains the advanced
override. A second global initialization raises a clear `RuntimeError`.
JSON tracing logs are distinct from JSON RenderEvent progress.

## Reports and errors

`ValidationReport` has `is_valid`, `diagnostics`, `errors` and `warnings`; `PreflightReport` adds `is_ready`. `Diagnostic` has `code`, `category`, `severity`, `message`, `pointer`, `related_id` and `hint`. Native runtime errors include `ProjectError`, `PreparationError`, `FrameRenderError`, `RenderError` and `PreparedProjectBusyError`. `BackendPreference` is `AUTO`, `CPU` or `WGPU`; `BackendKind` reports `CPU` or `WGPU`. The exact exported native declarations are in `python/vestra/_native.pyi`.
