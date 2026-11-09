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

## Export part of the timeline

```python
result = project.render("clip.mp4", start=0.5, end=1.0, overwrite=True)
```

`start` defaults to zero and an omitted `end` uses the project duration. Both
bounds are seconds on the original project timeline. The half-open interval
`[start, end)` selects frames whose sample times fall inside it: both boundaries
round up to frame indices at the project's rational FPS. A range must satisfy
`0 <= start < end <= duration` and contain at least one sampled frame.

For example, at 10 FPS, `[0.25, 0.68)` selects project frames 3–6. The output
contains four frames at timestamps 0, 0.1, 0.2 and 0.3 seconds, with a duration
of 0.4 seconds. Effects, transitions, source selection and audio-driven visuals
keep their original project time. Earlier output frames are not composed.

Audio is mixed with the original effects and automation, then trimmed from the
first selected frame's project time and rebased to zero. Audio boundaries round
to the nearest 48 kHz mixer sample. A final frame can extend beyond the authored
duration by less than one frame; the corresponding audio tail is silence.
Progress counts selected frames; failure frame indices refer to the project.
Range selection leaves the authored project unchanged.

The same selection works with reusable preparation:

```python
from vestra import PreparedVideoRenderRequest

prepared = project.prepare(backend="cpu")
result = prepared.render_video(
    PreparedVideoRenderRequest("prepared-clip.mp4", start=0.5, end=1.0, overwrite=True)
)
```

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

## Save a still image or sequence

```python
frame = project.render_frame(0.5, "poster.png", backend="cpu", overwrite=True)
frame.save("poster.webp", overwrite=True)
project.render_frame(frame_number=0, output="first.jpg", overwrite=True)
```

The extension selects PNG, JPEG (`.jpg` or `.jpeg`), or lossless static WebP.
PNG and WebP retain alpha; JPEG composites transparent pixels over black.
Still export uses built-in encoders and does not require the FFmpeg executable.
Each file is published after encoding succeeds, with the same overwrite
protection as video output.

For several images, share preparation automatically:

```python
count = project.render_frames(
    times=[0.5, 0, 0.5], directory="selected-frames", format="webp", overwrite=True,
)
count = project.render_frames(
    directory="sequence", start=0, end=0.5, overwrite=True,
)
```

`indices=[12, 0, 12]` selects exact project frame numbers instead of times.
Selection order and duplicates are retained. Files are named `frame_000000.webp`,
`frame_000001.webp`, and so on; the number refers to selection order. A continuous
sequence defaults to the whole timeline and includes frames whose rational
project times lie in `[start, end)`. Both boundaries round up to a frame index.
Invalid, empty or out-of-timeline ranges are refused. An explicitly empty list
writes nothing and returns zero. Existing numbered files are checked before
writing, and each file is published independently. A later encoding or I/O
failure leaves earlier completed images available.

Only one frame is encoded at a time. The return value is a count, so Vestra
does not retain a list of frames or output paths as the sequence grows.

## Use frames with NumPy

Install the optional extra with `pip install 'vestra[numpy]'`:

```python
pixels = project.get_frame(0.5, backend="cpu")
print(pixels.shape, pixels.dtype)  # (height, width, 4), uint8
pixels = frame.to_numpy()
```

Arrays contain RGBA8 with straight (unpremultiplied) alpha, top row first, in
contiguous row-major order. Their byte strides are `(width * 4, 4, 1)`. Each call
returns an independent writable copy that owns its memory, remains valid after
the frame or prepared project is released, and cannot alter the renderer.
Use `frame.to_bytes()` when NumPy is unnecessary. The existing `Frame` result
also records the selected project frame number and timestamp.

The [complete frame export example](../../assets/examples/frame_exports.py)
creates a project, stills, selected frames, a sequence and an array. Run it with
`python frame_exports.py` after installing `vestra[numpy]`. It replaces its
outputs inside `frame-exports/` on each run.

## Render videos from prepared state

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
