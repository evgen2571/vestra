# Render and prepare from Python

For a normal render, use the high-level one-shot method:

```python
result = project.render(
    "output.mp4",
    backend="cpu",
    overwrite=True,
    preview=False,
)
print(result.output_path, result.selected_backend)
```

Use `project.validate()` when you want a report without preparing a renderer.
Use `project.prepare(backend="auto")` when you will render frames or a video
through the prepared runtime more than once. `project.render_frame(seconds)`
is useful for a lightweight inspection of one time position.

The lower-level immutable route is exposed through `ProjectSnapshot` and
`Editor`:

```python
import vestra

snapshot = project.snapshot(output="output.mp4")
editor = vestra.Editor()
prepared = editor.prepare(
    snapshot, vestra.PrepareOptions(backend=vestra.BackendPreference.CPU)
)
frame = prepared.render_frame_seconds(0.5)
```

Use `RenderRequest` with `Editor.render(...)` for one-shot rendering. Prepared
rendering uses `PreparedVideoRenderRequest`, which contains the output path and
overwrite policy. A callback receives `RenderEvent` values. The returned
`RenderResult` reports the requested and selected backend and may include
adapter information. A requested WGPU backend does not by itself prove
hardware rendering, so inspect the actual result.

Cancellation raises `vestra.CancelledError` from Python render operations. It
does not return a successful `RenderResult`.

Preparation is the point where validation, preflight, plan compilation, media
and audio analysis, and renderer setup become ready for evaluation. Keep
`ProjectBuilder` for advanced canonical authoring rather than making it the
default route for ordinary editing.
