---
description: Render videos and previews, export stills or arrays, and reuse prepared projects from Python.
---

# Render and prepare from Python

Start with a `project` from the [first render tutorial](../../getting-started/first-render.md).
The examples below use that project to export videos, images and arrays.
Use `prepare()` when you need several outputs from the same project.

## Encode one video

```python
result = project.render(
    "output.mp4",
    backend="cpu",
    overwrite=True,
)
print(f"Saved {result.output_path} using {result.selected_backend}")
```

This replaces `output.mp4` if it exists. Omit `overwrite=True` to protect an
existing file. A successful result means the file has finished encoding and is
available at `result.output_path`.

Use `backend="auto"` to prefer WGPU when an adapter is available and fall back
to CPU otherwise. Check the result's backend and adapter information to see
what ran; a WGPU adapter may use software. See [backends](../../reference/backends.md).

## Render a preview or change output settings

For a quick draft:

```python
project.render_preview("preview.mp4", overwrite=True)
```

The preview keeps all effects, caps the longest side at 640 pixels and the
frame rate at 15 FPS, and uses fast H.264 encoding. It never increases a smaller
project's dimensions or a lower frame rate. `project.render(..., preview=True)`
uses the same preset.

To choose the settings yourself:

```python
from vestra.authoring import Quality

project.render(
    "small.mp4",
    resolution=(640, 360),
    fps=(24000, 1001),
    quality=Quality.BALANCED,
    overwrite=True,
)
```

These settings leave the authored project unchanged. Positions and sizes keep
their authored proportions; a different aspect ratio fits the composition in
the center without stretching it. Explicit settings override the corresponding
preview defaults. Quality controls apply to MP4.

Lower resolution reduces composition and effect work, but source decoding
still runs at native resolution. See [output settings](../../reference/render-outputs.md#output-settings)
for accepted values, encoder presets and effect limits.

## Export part of the timeline

```python
result = project.render("clip.mp4", start=0.5, end=1.0, overwrite=True)
```

`start` and `end` are seconds on the original project timeline. Omit `end` to
render through the project duration. The selection includes frames at or after
`start` and before `end`; it must contain at least one frame.

Effects and transitions keep their original project time. The exported clip
starts at timestamp zero, with audio trimmed and synchronized to the selected
frames. For example, at 10 FPS, `start=0.25, end=0.68` selects project frames
3–6 and produces a four-frame clip lasting 0.4 seconds.

See [range boundaries](../../reference/render-outputs.md#ranges-and-frame-boundaries)
for exact rounding, audio tails and error behavior.

## Export GIF or animated WebP

```python
project.render("animation.gif", fps=15, start=0, end=0.5, overwrite=True)
project.render("animation.webp", fps=24, start=0, end=0.5, overwrite=True)
```

The extension selects the format. Both animations loop indefinitely and contain
no audio stream. Audio-driven visuals still work, and audio assets still need
preflight when they determine the project duration or supply visual analysis.

Use GIF for palette-based animation with fully opaque or fully transparent
pixels. Animated WebP preserves partial transparency and uses lossless frames.
Vestra streams both formats without collecting the whole animation in memory.
See [animation formats](../../reference/render-outputs.md#animation-formats)
for color, timing, encoder and file-size limits.

## Save a still image or sequence

```python
frame = project.render_frame(0.5, "poster.png", backend="cpu", overwrite=True)
frame.save("poster.webp", overwrite=True)
project.render_frame(frame_number=0, output="first.jpg", overwrite=True)
```

Choose PNG, JPEG or static WebP by extension. PNG and WebP preserve alpha;
JPEG composites transparent pixels over black. Still export uses built-in
encoders and does not require the FFmpeg executable. `frame.save("poster.webp")`
writes a static image; `project.render("animation.webp")` writes an animation.

For selected images, supply times or zero-based frame indices:

```python
count = project.render_frames(
    times=[0.5, 0, 0.5], directory="selected-frames", format="webp", overwrite=True,
)
```

Selection order and duplicates are retained. Files are named `frame_000000.webp`,
`frame_000001.webp`, and so on, in selection order. Use `indices=[12, 0, 12]`
instead of `times` to select exact frame numbers.

For a continuous sequence:

```python
count = project.render_frames(
    directory="sequence", start=0, end=0.5, overwrite=True,
)
```

Omit both bounds to export the whole timeline. The return value is the number
of saved images. Preparation is shared and only one frame is encoded at a time.
Each image is published independently; a later failure leaves earlier completed
images available. See [image sequences](../../reference/render-outputs.md#image-sequences)
for range selection and overwrite checks.

Frame methods and sequences also accept `resolution` and `fps`:

```python
frame = project.render_frame(0.5, resolution=(320, 180), backend="cpu")
project.render_frames(
    directory="small-sequence", resolution=(320, 180), fps=12, overwrite=True,
)
```

## Use frames with NumPy

Install the optional extra with `pip install 'vestra[numpy]'`:

```python
pixels = project.get_frame(0.5, backend="cpu")
print(pixels.shape, pixels.dtype)  # (height, width, 4), uint8
pixels = frame.to_numpy()
```

Each array is an independent writable copy of the frame's RGBA pixels, with
straight alpha and the top row first. It remains valid after the frame or
prepared project is released. Use `frame.to_bytes()` when NumPy is unnecessary.
See [frame data](../../reference/render-outputs.md#frames-and-arrays) for layout
and ownership details.

The [complete frame export example](https://raw.githubusercontent.com/evgen2571/vestra/develop/examples/tutorials/frame_exports.py)
creates a project, stills, selected frames, a sequence and an array. Run it with
`python frame_exports.py` after installing `vestra[numpy]`. It replaces its
outputs inside `frame-exports/` on each run.

## Validate before rendering

```python
report = project.validate()
if not report.is_valid:
    for diagnostic in report.diagnostics:
        print(diagnostic)
```

Validation checks project structure and relationships. Preparation and rendering
also check media and the environment; validation alone does not establish that
a render can succeed. CLI `ve validate` includes environment preflight. The
[rendering lifecycle](../../concepts/rendering-lifecycle.md) explains these stages.

## Prepare once for several frames

```python
prepared = project.prepare(backend="cpu")
first_frame = prepared.render_frame_number(0)
later_frame = prepared.render_frame_seconds(0.5)
```

Both frames use one prepared snapshot. Later authoring edits do not affect it;
prepare again to include them. In contrast, each `project.render_frame()` call
prepares again. Neither route encodes video or checks output publication.

You can pass `resolution`, `fps` and `quality` to `prepare()`. The prepared
runtime keeps those settings for its lifetime; prepare again to change them.

## Render videos from prepared state

The same prepared runtime can produce several videos without rebuilding resources:

```python
from vestra import PreparedVideoRenderRequest

request = PreparedVideoRenderRequest("prepared-output.mp4", overwrite=True)
result = prepared.render_video(request)
```

Each request supplies a destination and overwrite policy. Add `start` and `end`
to select a timeline interval, or use a `.gif` or `.webp` destination for an
animation. The prepared runtime already owns its snapshot and renderer.

## Progress and cancellation

`project.render()` shows progress automatically when appropriate for the terminal.
Set `show_progress=False` to disable it, or use `on_progress=callback` instead.
Callbacks receive typed `started`, `stage_changed`, `progress` and terminal
outcome events. `completed` follows successful publication; a frame-progress
fraction of `1.0` is an earlier milestone. Use the event kind and stage rather
than assuming every event has a fraction.

Pass a `CancellationToken` as `cancellation=token` and call `token.cancel()` from
controlling code to request cooperative cancellation. Python raises
`vestra.CancelledError` instead of returning a successful result.

If a callback raises before terminal success, Vestra stops rendering and
re-raises the original Python exception. A native cleanup error is attached as
`render_cleanup_error`. A callback failure while receiving `completed` cannot
invalidate an already-published render. See [runtime events and errors](../../reference/python-api.md#progress-and-cancellation)
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

Use `RenderRequest` with `Editor.render(...)` for one-shot rendering, or
`PreparedVideoRenderRequest` with an existing runtime. `ProjectBuilder` is for
direct canonical authoring. See the [Python API reference](../../reference/python-api.md#native-runtime)
for signatures and report fields.
