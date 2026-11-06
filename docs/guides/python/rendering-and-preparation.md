# Render and prepare from Python

With a `project` built using the [first render tutorial](../../learn/first-render.md),
use the high-level method to encode an MP4:

```python
result = project.render(
    "output.mp4",
    backend="cpu",
    overwrite=True,
)
print(f"Saved {result.output_path} using {result.selected_backend}")
```

Use `project.validate()` when you want a report without preparing a renderer.
Use `project.prepare(backend="auto")` when you will render frames or a video
through the prepared runtime more than once. `project.render_frame(seconds)`
is useful for a lightweight inspection of one time position.

The lower-level immutable route is exposed through `ProjectSnapshot` and
`Editor`:

```python
from vestra import BackendPreference, Editor, PrepareOptions

snapshot = project.snapshot(output="output.mp4")
editor = Editor()
options = PrepareOptions(backend=BackendPreference.CPU)
prepared = editor.prepare(snapshot, options)
frame = prepared.render_frame_seconds(0.5)
```

Use `RenderRequest` with `Editor.render(...)` for one-shot rendering. Prepared
rendering uses `PreparedVideoRenderRequest`, which contains the output path and
overwrite policy. `project.render("output.mp4")` uses native Auto progress by
default; `show_progress=False` disables it and `on_progress=callback` replaces
it with a callback. Callbacks receive RenderEvent v1 values for `started`,
`stage_changed`, `progress`, and the terminal outcome. `completed` occurs only
after encoder finalization and output publication. The returned `RenderResult`
reports the requested and selected backend and may include adapter information.
A requested WGPU backend does not by itself prove hardware rendering, so inspect
the actual result.

Cancellation raises `vestra.CancelledError` from Python render operations. It
does not return a successful `RenderResult`.

If the progress callback raises, Vestra stops rendering and re-raises that same
Python exception. It does not convert it to `RenderError` or return a result.
If native cleanup also reports an error, it is attached to the original
exception as `render_cleanup_error`.

Preparation is the point where validation, preflight, plan compilation, media
and audio analysis, and renderer setup become ready for evaluation. Keep
`ProjectBuilder` for advanced canonical authoring rather than making it the
default route for ordinary editing.
