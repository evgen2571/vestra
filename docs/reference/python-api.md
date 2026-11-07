# Python API

Use `vestra.Project` to author a mutable project. Create an immutable
`ProjectSnapshot` (`vestra._native.Project`) to save canonical JSON or run native
operations through `Editor`. The advanced `vestra.authoring.ProjectBuilder`
constructs canonical objects directly.

## High-level authoring

```text title="Project constructor"
Project(*, size: tuple[int, int], fps: int | tuple[int, int] | FrameRate,
        duration: float | None = None, background="#000000", quality=Quality.BALANCED,
        base_directory=".", name=None, metadata=None, output_audio=None, output_path=None,
        resource_limits: ResourceLimits | None = None)
```

| Parameter | Contract |
| --- | --- |
| `size` | Positive integer width and height in pixels; canonical output limits also apply. |
| `fps` | Integer, `(numerator, denominator)` pair or `FrameRate`. |
| `duration` | Positive seconds when explicit; `None` uses automatic duration. |
| `output_audio` | `None` follows whether authored audio clips exist; an explicit boolean overrides publication. |
| `output_path` | Optional default for snapshots; rendering takes an explicit output. |

`project.root` is the top-level `Composition`. Source constructors are documented
on the [source pages](catalog.md#source-types); `list_system_fonts() -> list[str]` lists
installed families without a project. See [Text](sources/text.md) for font selection.

```text title="Layer placement signature"
composition.add(source, *, start=0, duration=None, source_start=0.0,
                playback_rate=1.0, z=0, visible=True, opacity=1.0,
                id=None, name=None, blend_mode=BlendMode.NORMAL,
                transform=None, position=None, anchor=None, scale=None,
                rotation=None, rotation_degrees=None) -> Layer
```

`add()` copies the source. `start` and `duration` use owning-composition seconds.
Omitted duration uses the owning duration except for `Video`, where placement
probes media to derive the usable source span. `source_start` uses source-media
seconds; `playback_rate` is positive. Later edits belong on the returned layer.

### Nested authoring

```text title="Owned composition placement signature"
composition.group(name=None, *, start=0, duration=None, z=0, visible=True,
                  opacity=1, id=None, blend_mode=BlendMode.NORMAL) -> CompositionLayer
```

`CompositionLayer.child` is another composition with child-local timing.
A reusable `Group` is an ordinary visual source:

```text title="Reusable group source and child signatures"
Group(children=(), *, size=None, transform=None, position=None, anchor=None, scale=None,
      rotation=None, rotation_degrees=None)
group.add(source, *, start=0, duration=None, source_start=0, playback_rate=1,
          layer=None, visible=True, opacity=1, transform=None, position=None,
          anchor=None, scale=None, rotation=None, rotation_degrees=None) -> GroupChild
```

The optional read-only `size: tuple[float, float]` contains positive finite pixel
dimensions. Omit it to inherit the containing canvas. Size changes coordinates,
without scaling child geometry or changing timing/FPS. See
[nested groups](../guides/python/nested-compositions.md) for ownership and examples.

### Source presentation

All visual source constructors accept the same transform keywords; ParticleSystem
keeps `rotation` for particle orientation and accepts `rotation_degrees` for
presentation. Sources expose `transform` defaults; placements copy them.
An individual placement override replaces only that property's animation and
bindings. `transform=` replaces all defaults and is exclusive with individual
keywords. See [precedence and validation](../guides/python/sources-and-layers.md).

### Masks and track mattes

```text title="Owned mask collection signatures"
layer.masks.add(source, *, operation=MaskOperation.INTERSECT, id=None, feather=0, mode=None)
layer.masks.items
layer.masks.remove(mask_or_id)
layer.masks.clear()
```

A `Mask` exposes `id`, `input`, `operation`, `invert`, `strength`, `feather` and
`transform`. Shapes, Image, Color, Text, Video, Spectrum2D, ParticleSystem and
owned Group sources are accepted. Non-shapes select `MaskCoverageMode.ALPHA`
(default) or `LUMA`; shapes use rendered alpha and reject `mode`. Direct Image
and Video masks reject sizing/crop. Strength, feather and transform properties
support keyframes, modifiers and signals. Masks follow layer effects and precede
final opacity/blending.

```text title="Sibling matte signatures"
layer.set_matte(matte, *, mode=MatteMode.ALPHA, invert=False)
layer.clear_matte()
```

The matte must be a layer in the same immediate composition. See
[masks and track mattes](masks.md) for coordinates, source restrictions and timing.

### Advanced canonical authoring

```text title="ProjectBuilder constructor"
ProjectBuilder(*, width, height, frame_rate, output_path, duration=None,
               duration_mode=None, background="#000000", quality=Quality.BALANCED,
               base_directory=".", name=None, metadata=None, output_audio=False)
```

The builder owns assets, clips, tracks, effects, transitions, flashes and audio.
`build()` creates a native snapshot; `validate()` performs semantic validation only.

## Snapshots and validation

```text title="High-level snapshot and validation signatures"
project.snapshot(*, output=None) -> ProjectSnapshot
project.validate() -> ValidationReport
```

Snapshots are immutable copies; later authoring edits do not change them.
`snapshot()` defaults its output to the construction-time path or `"output.mp4"`.
Lowering resolves authored font selections. `validate()` then performs canonical
semantic validation only; it does not probe media or create a renderer.

```text title="Snapshot load/save signatures"
ProjectSnapshot.load(path)
ProjectSnapshot.from_json(text, *, base_directory=None)
ProjectSnapshot.from_dict(data, *, base_directory=None)
snapshot.to_json()
snapshot.to_dict()
snapshot.save(path)
```

<span id="native-runtime"></span>

## Preparation and rendering

```text title="High-level rendering signatures"
project.prepare(*, backend="auto") -> PreparedProject
project.render_frame(seconds, *, backend="auto") -> Frame
project.render(output, *, backend="auto", overwrite=False, preview=False,
               show_progress=True, on_progress=None,
               cancellation=None) -> RenderResult
```

`render_frame()` prepares on every call. Reuse `prepare()` for repeated frames or
video renders. Backend preferences are `"auto"`, `"cpu"`, `"wgpu"` or
`BackendPreference` values; see [backends](backends.md). Existing outputs require
`overwrite=True`.

```text title="Native Editor signatures (project is a ProjectSnapshot)"
Editor(*, resource_limits=None)
editor.validate(project)
editor.preflight(project, options)
editor.inspect(project, *, preview=False)
editor.prepare(project, options=None)
editor.render(project, request, *, show_progress=True, on_progress=None, cancellation=None)
```

| Options/request type | Construction and purpose |
| --- | --- |
| `PrepareOptions` | `PrepareOptions(*, backend=None)` selects preparation backend. |
| `RenderRequest` | `RenderRequest(output, *, backend=None, overwrite=False, preview=False)` for one-shot `Editor.render`. |
| `PreparedVideoRenderRequest` | `PreparedVideoRenderRequest(output, *, overwrite=False)` for prepared video rendering. |

`Editor.validate()` checks canonical semantics only. Choose a preflight target
for the additional operational checks below; backend arguments use `BackendPreference`.

| `PreflightOptions` constructor | Additional checks |
| --- | --- |
| `for_validation()` | Media, tools, configured output with overwrite disabled, and Auto backend readiness; matches `ve validate`. |
| `for_inspection()` | Media, without output/backend checks. |
| `for_preparation(backend)` | Media and requested backend readiness. |
| `for_render(backend, output, *, overwrite=False)` | Media, backend, supplied destination and render policy. |

```text title="Prepared frame and video signatures"
prepared.render_frame_number(frame_number)
prepared.render_frame_ns(timestamp_ns)
prepared.render_frame_seconds(seconds)
prepared.render_video(request, *, on_progress=None, show_progress=True, cancellation=None)
```

## Resource limits

```text title="Resource budget constructor"
ResourceLimits(*, maximum_group_width=None, maximum_group_height=None,
               maximum_group_pixels=None)
```

`ResourceLimits` configures group canvas budgets. Omitted or `None`
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

## Progress and cancellation

Rendering uses native Auto terminal progress by default. Set `show_progress=False`
to disable it, or pass `on_progress=my_callback` to replace the built-in renderer.

```text title="Cancellation token signature"
CancellationToken()
token.cancel()
token.is_cancelled  # read-only
```

`RenderEvent` v1 exposes `schema_version`, `kind`, `operation_id`, `stage`,
`frame`, `total_frames`, `fraction`, and `output_path` with the applicable
fields optional. Its lifecycle is `started`, `stage_changed`, `progress`, and
one terminal event: `completed`, `cancelled`, or `failed`. `Completed` occurs only
after encoder finalization and
successful output publication. Rendering may reach fraction `1.0` before
encoding and finalizing complete. A callback error before terminal success may
abort the operation; an error while receiving `Completed` cannot invalidate a
successfully published render. `CancelledError` is a `RenderError`; a
cancelled operation does not return a `RenderResult`.

If a Python progress callback raises, Vestra stops the native render and
re-raises the original Python exception. No `RenderResult` is returned. If
cleanup reports a native error, it is attached as
`error.render_cleanup_error`; it does not replace the callback exception.

### Successful results and memory counters

`RenderResult` on success exposes output dimensions/timing, selected/requested backend, fallback, adapter, warnings,
detailed `timings` and `performance`.

`performance` exposes CPU scratch backing capacity, native video cache/current/
working-buffer bytes, per-session cache peaks, and worker/output memory estimates.
See [CPU memory accounting](../development/architecture/cpu-renderer.md#memory-accounting)
for ownership, exclusions and units. These byte fields are not process RSS/PSS.

## Logging

Logging is explicit and uses the shared Rust observability implementation;
importing Vestra and ordinary rendering do not install a global subscriber:

```python
import vestra

vestra.configure_logging(level="info")
```

For JSON logs in a file, choose this configuration instead. Initialize logging
once per process:

```python
import vestra

vestra.configure_logging(
    level="debug",
    format="json",
    output="file",
    file="vestra.log",
)
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

| Value | Fields or meaning |
| --- | --- |
| `ValidationReport` | `is_valid`, `diagnostics`, `errors`, `warnings`. |
| `PreflightReport` | The validation fields plus `is_ready`. |
| `Diagnostic` | `code`, `category`, `severity`, `message`, `pointer`, `related_id`, `hint`; see [diagnostics](diagnostics.md). |
| Native errors | `ProjectError`, `PreparationError`, `FrameRenderError`, `RenderError`, `PreparedProjectBusyError`; cancellation raises `CancelledError`, a `RenderError`. |
| Backend enums | `BackendPreference`: `AUTO`, `CPU`, `WGPU`; selected `BackendKind`: `CPU`, `WGPU`. |

The exact native exports and declarations are in
[`python/vestra/_native.pyi`](https://github.com/evgen2571/vestra/blob/develop/python/vestra/_native.pyi).
