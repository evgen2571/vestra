---
description: Export a video, inspect frames efficiently and handle progress or cancellation through the Python runtime.
---

# Render and prepare from Python

Start with a `project` from the [first render tutorial](../../learn/first-render.md).
Choose a one-shot render for an output file or a prepared runtime for repeated
inspection. The [rendering lifecycle](../../concepts/rendering-lifecycle.md)
explains what each stage establishes.

## Encode one video

```python
result = project.render(
    "output.mp4",
    backend="cpu",
    overwrite=True,
)
print(f"Saved {result.output_path} using {result.selected_backend}")
```

This intentionally replaces `output.mp4` if it exists. Without `overwrite=True`,
an existing destination is refused. Returning a result means encoding,
finalization and publication succeeded; a frame-progress fraction of `1.0` is
an earlier milestone.

`backend="auto"` prefers WGPU when an adapter is available, otherwise CPU.
Requesting WGPU alone does not prove hardware rendering: inspect the selected
backend and adapter information in the result. See [backends](../../reference/backends.md).

## Validate before rendering

```python
report = project.validate()
if not report.is_valid:
    for diagnostic in report.diagnostics:
        print(diagnostic)
```

Validation checks semantic structure and relationships. It does not check media
files, FFmpeg, the output path or graphics adapters. Preparation and rendering
include environment preflight. CLI `ve validate` includes its validation preflight
target, so it can catch environment problems that this Python call does not.

## Prepare once for several frames

```python
prepared = project.prepare(backend="cpu")
first_frame = prepared.render_frame_number(0)
later_frame = prepared.render_frame_seconds(0.5)
```

Both frames use one prepared snapshot. Later authoring edits do not affect it;
prepare again to include them. For one quick inspection,
`project.render_frame(0.5, backend="cpu")` is convenient, but every call prepares
again. Neither route encodes video or verifies output publication.

A prepared runtime can also produce several videos without rebuilding it:

```python
from vestra import PreparedVideoRenderRequest

request = PreparedVideoRenderRequest("prepared-output.mp4", overwrite=True)
```

Call `prepared.render_video(request)` when ready to encode that file. The request
supplies output and overwrite policy; the prepared runtime already owns its
snapshot and renderer.

## Progress and cancellation

`project.render()` shows native Auto progress by default. Set
`show_progress=False` to disable it, or use `on_progress=callback` instead.
Callbacks receive typed `started`, `stage_changed`, `progress` and terminal
outcome events. `completed` follows successful publication. Callback handlers
should use the event kind and stage rather than assuming every event has a
fraction.

Pass a `CancellationToken` as `cancellation=token` to a render operation and call
`token.cancel()` from controlling code to request cooperative cancellation.
Python raises `vestra.CancelledError`; it does not return a successful result.

If a callback raises before terminal success, Vestra stops rendering and
re-raises the original Python exception. A native cleanup error is attached as
`render_cleanup_error`. A callback failure while receiving `completed` cannot
invalidate an already-published render. See [runtime events and errors](../../reference/python-api.md#native-runtime)
for the exact integration contract.

## Use the immutable runtime explicitly

Use this route when a canonical snapshot is already part of your workflow:

```python
from vestra import BackendPreference, Editor, PrepareOptions

snapshot = project.snapshot(output="output.mp4")
editor = Editor()
options = PrepareOptions(backend=BackendPreference.CPU)
prepared = editor.prepare(snapshot, options)
frame = prepared.render_frame_seconds(0.5)
```

`RenderRequest` with `Editor.render(...)` provides one-shot rendering;
`PreparedVideoRenderRequest` supplies output settings to an existing runtime.
Keep `ProjectBuilder` for exact canonical authoring rather than ordinary editing.
See [Python API reference](../../reference/python-api.md#native-runtime) for the
full signatures and report fields.
