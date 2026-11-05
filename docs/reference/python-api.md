# Python API

`vestra.Project` is the normal mutable authoring API. `vestra.authoring.ProjectBuilder` is the advanced canonical authoring API. Runtime control uses immutable native `ProjectSnapshot` (`vestra._native.Project`) and `Editor`.

## High-level authoring

```python
Project(*, size: tuple[int, int], fps: int | tuple[int, int] | FrameRate,
        duration: float | None = None, background="#000000", quality=Quality.BALANCED,
        base_directory=".", name=None, metadata=None, output_audio=None, output_path=None,
        resource_limits: ResourceLimits | None = None)

composition.add(source, *, start=0, duration=None, source_start=0.0,
                playback_rate=1.0, z=0, visible=True, opacity=1.0,
                id=None, name=None, blend_mode=BlendMode.NORMAL,
                transform=None, position=None, anchor=None, scale=None,
                rotation=None, rotation_degrees=None) -> Layer
composition.group(name=None, *, start=0, duration=None, z=0, visible=True,
                  opacity=1, id=None, blend_mode=BlendMode.NORMAL) -> CompositionLayer
```

`size` has positive integer width/height; `fps` is an integer, `(numerator, denominator)` pair or `FrameRate`; explicit `duration` is positive seconds. `root` is the top-level `Composition`; a `CompositionLayer.child` is another composition with child-local timing. `add` copies its `Source`; `duration=None` uses the owning duration except for `Video`, where it probes media to derive the usable source duration. `source_start` is source-media seconds and `playback_rate` is positive.

`Group(children=(), *, size=None, transform=None, position=None, anchor=None, scale=None,
rotation=None, rotation_degrees=None)` is an ordinary visual source.
Its optional `size: tuple[float, float]` contains positive, finite pixel
dimensions and is exposed as a read-only property. Omit it to inherit the
containing coordinate system. Size changes child coordinates without scaling
their geometry or changing timing/FPS.
`group.add(source, *, start=0, duration=None, source_start=0, playback_rate=1,
layer=None, visible=True, opacity=1, transform=None, position=None, anchor=None,
scale=None, rotation=None, rotation_degrees=None)` returns a `GroupChild`.
See [nested groups](../guides/python/nested-compositions.md) for ownership,
local timing and the direct `CompositionLayer` alternative.

## Resource limits

`ResourceLimits(*, maximum_group_width=None, maximum_group_height=None,
maximum_group_pixels=None)` configures group canvas budgets. Omitted or `None`
values use safe defaults: 8192 pixels per axis and 67,108,864 pixels per canvas.
Values are nonnegative integers within the native `u32`/`u64` ranges; the three
properties are read-only. Zero disallows a positive canvas under that budget.
Other native resource budgets retain their defaults.

Pass the value to `Project(..., resource_limits=limits)` for high-level
validation, preparation and one-shot rendering, or to
`Editor(resource_limits=limits)` when operating on immutable snapshots. These
settings are not serialized in project JSON. Canonical dimensions must remain
positive, finite and safely representable; CPU allocation constraints and WGPU
device limits still apply when budgets are raised.

## Source presentation

All visual source constructors accept the same transform keywords; ParticleSystem
keeps `rotation` for particle orientation and accepts `rotation_degrees` for
presentation. Sources expose `transform` defaults; placements copy them.
An individual placement override replaces only that property's animation and
bindings. `transform=` replaces all defaults and is exclusive with individual
keywords. See [precedence and validation](../guides/python/sources-and-layers.md).

```python
project.snapshot(*, output=None) -> ProjectSnapshot
project.validate() -> ValidationReport
project.prepare(*, backend="auto") -> PreparedProject
project.render_frame(seconds, *, backend="auto") -> Frame
project.render(output, *, backend="auto", overwrite=False, preview=False,
               show_progress=True, on_progress=None,
               cancellation=None) -> RenderResult
```

`project.render("output.mp4")` uses native Auto progress by default. Set
`show_progress=False` to disable it, or pass `on_progress=my_callback` to
replace the built-in terminal renderer with a callback.

`list_system_fonts() -> list[str]` lists installed font families without a project.
`Text(..., font=None)` uses the bundled default; see [Text](sources/text.md).

`validate()` lowers then performs canonical semantic validation only. It resolves authored font selections during lowering, but does not probe media or create a renderer. `snapshot()` defaults output to the construction-time output path or `"output.mp4"`; `output_audio=None` follows whether authored audio clips exist. See individual [source pages](sources/image.md) for source constructors.

Layers expose an ordered mask collection:

```python
mask = layer.masks.add(source, operation=MaskOperation.INTERSECT,
                       id=None, feather=12, mode=None)
layer.masks.items
layer.masks.remove(mask_or_id)
layer.masks.clear()
```

`Mask` exposes `id`, `input`, `operation`, `invert`, `strength`, `feather`, and
`transform`. Inputs include `Rectangle`, `Ellipse`, `Circle`, `Polygon`, `Line`,
`Image`, `Color`, `Text`, `Video`, `Spectrum2D`, `ParticleSystem`, and owned
`Group` sources. `mode` selects `MaskCoverageMode.ALPHA` (default) or `LUMA`
for non-shape sources; shapes use their rendered alpha and reject `mode`.
Direct Image/Video masks reject sizing and crop; use `mask.transform`.
`strength`, `feather`, and transform properties support keyframes, modifiers
and signal bindings. Masks are applied after layer effects and before final
opacity/blending.

`layer.set_matte(matte, *, mode=MatteMode.ALPHA, invert=False)` references
a layer in the same immediate composition; `layer.clear_matte()` removes it.
See [masks and track mattes](masks.md) for coverage, timing and visibility rules.

`ProjectBuilder(*, width, height, frame_rate, output_path, duration=None, duration_mode=None, background="#000000", quality=Quality.BALANCED, base_directory=".", name=None, metadata=None, output_audio=False)` owns advanced canonical assets, visual clips, tracks, effects, transitions, flashes and audio. `build()` creates a native snapshot; `validate()` has the same semantic-only meaning.

## Native runtime

| Type | Exact construction or methods |
| --- | --- |
| `ProjectSnapshot` | `load(path)`, `from_json(text, *, base_directory=None)`, `from_dict(data, *, base_directory=None)`, `to_json()`, `to_dict()`, `save(path)`. |
| `Editor` | `Editor(*, resource_limits=None)`, then `validate(project)`, `preflight(project, options)`, `inspect(project, *, preview=False)`, `prepare(project, options=None)`, `render(project, request, *, show_progress=True, on_progress=None, cancellation=None)`. |
| `PreflightOptions` | Class methods `for_validation()`, `for_inspection()`, `for_preparation(backend)`, `for_render(backend, output, *, overwrite=False)`. Backend arguments use `BackendPreference`. |
| `PrepareOptions` | `PrepareOptions(*, backend=None)`. |
| `PreparedProject` | `render_frame_number(frame_number)`, `render_frame_ns(timestamp_ns)`, `render_frame_seconds(seconds)`, `render_video(request, *, on_progress=None, show_progress=True, cancellation=None)`. |
| `RenderRequest` | `RenderRequest(output, *, backend=None, overwrite=False, preview=False)` for one-shot `Editor.render`. |
| `PreparedVideoRenderRequest` | `PreparedVideoRenderRequest(output, *, overwrite=False)` for `PreparedProject.render_video`. |
| `CancellationToken` | `CancellationToken()`, `cancel()`, read-only `is_cancelled`. |

`Editor` methods take immutable `ProjectSnapshot` values. `validate()` checks
canonical semantics only. `preflight(..., PreflightOptions.for_validation())`
also checks media, tools, the configured output path with overwrite disabled,
and Auto backend readiness, matching `ve validate`. `for_inspection()` resolves
media without output/backend checks; `for_preparation(backend)` adds backend
readiness; `for_render(backend, output, overwrite=...)` checks the supplied
destination and render policy.

`RenderEvent` v1 exposes `schema_version`, `kind`, `operation_id`, `stage`,
`frame`, `total_frames`, `fraction`, and `output_path` with the applicable
fields optional. Its lifecycle is `started`, `stage_changed`, `progress`, and
one terminal event: `completed`, `cancelled`, or `failed`. Pass `on_progress`
for a callback, or set `show_progress=False` to disable
the built-in Auto terminal presentation. Custom callbacks replace the built-in
terminal renderer. `Completed` occurs only after encoder finalization and
successful output publication. Rendering may reach fraction `1.0` before
encoding and finalizing complete. A callback error before terminal success may
abort the operation; an error while receiving `Completed` cannot invalidate a
successfully published render. A successful `RenderResult` exposes output
dimensions/timing, selected/requested backend, fallback, adapter, warnings,
detailed `timings` and `performance`. `CancelledError` is a `RenderError`; a
cancelled operation does not return a `RenderResult`.

`performance` exposes CPU scratch backing capacity, native video cache/current/
working-buffer bytes, per-session cache peaks, and worker/output memory estimates.
See [CPU memory accounting](../development/architecture/cpu-renderer.md#memory-accounting)
for ownership, exclusions and units. These byte fields are not process RSS/PSS.

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
vestra.configure_logging(level="debug", format="json", output="file",
                         file="vestra.log")
```

`level` accepts `error`, `warn`, `info`, `debug`, and `trace`. `format` is
`human` or `json`. `output` is `stderr` (the default), `file`, or
`stderr_and_file`. `file` is required for `file` and `stderr_and_file`, and
must be omitted for `stderr`; passing a file with `output="stderr"` is an
error. `filter=` accepts an EnvFilter directive such as
`"vestra=info,vestra.render.wgpu=debug"`; `RUST_LOG` remains the advanced
override. A second global initialization raises a clear `RuntimeError`.
JSON tracing logs are distinct from JSON RenderEvent progress. JSON tracing on
stderr disables native Auto or explicit terminal progress on that shared
terminal so the stream stays valid JSON Lines; JSON tracing to a file leaves
native stderr progress available.

## Reports and errors

`ValidationReport` has `is_valid`, `diagnostics`, `errors` and `warnings`; `PreflightReport` adds `is_ready`. `Diagnostic` has `code`, `category`, `severity`, `message`, `pointer`, `related_id` and `hint`. Native runtime errors include `ProjectError`, `PreparationError`, `FrameRenderError`, `RenderError` and `PreparedProjectBusyError`. `BackendPreference` is `AUTO`, `CPU` or `WGPU`; `BackendKind` reports `CPU` or `WGPU`. The exact exported native declarations are in `python/vestra/_native.pyi`.
