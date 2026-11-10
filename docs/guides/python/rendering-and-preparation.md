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
round up to frame indices at the selected output's rational FPS. A range must satisfy
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

## Render a preview or change output settings

```python
from vestra.authoring import Quality

project.render_preview("preview.mp4", overwrite=True)
project.render(
    "small.mp4", resolution=(640, 360), fps=(24000, 1001),
    quality=Quality.BALANCED, overwrite=True,
)
```

The preview preset caps the longest side at 640 pixels, caps FPS at 15, and
uses `Quality.PREVIEW` (H.264 CRF 30 with the `ultrafast` speed preset).
`project.render(..., preview=True)` is equivalent. Explicit `resolution`, `fps`
and `quality` take precedence over their respective preset defaults. Preview
never increases a smaller project's dimensions or a lower frame rate.
`Quality.BALANCED` uses CRF 23 / `veryfast`; `Quality.HIGH` uses CRF 18 / `medium`.
These encoder controls apply to MP4; GIF quantization and lossless WebP use
the format policies below.

Overrides leave the authored project unchanged. `fps` accepts a positive integer,
an `(numerator, denominator)` tuple or `vestra.FrameRate`. Frame indices and
range boundaries use the selected output FPS; effects and keyframes continue
to evaluate at the corresponding original project time. A lower FPS requests
fewer output frames, including when rendering a selected interval.

`resolution=(width, height)` accepts even integers in `2..=8192`, within
configured resource limits, and changes actual composition surfaces and raster
work. Source sizing, positions, rotations, groups, masks and pixel-valued effect
sizes use the authored coordinate space. A different aspect ratio fits the
composition centrally without stretching it; uncovered pixels use the project
background. Global effects execute on the final canvas, including these borders.
Small kernels and stylization cells round to the renderer's pixel grid. Upscaling
can exceed an effect's existing physical kernel/cell limits; such requests fail
with an explicit render diagnostic rather than silently clipping the effect.
Changing aspect ratio with both a partially transparent background and a
non-normal root blend mode is currently refused; use matching aspect ratios
or an opaque/fully transparent background for that combination.
Source decoding and prepared source rasters retain their native resolution, so
preview speedups are smaller when decoding dominates.

Frame access and sequences support the same spatial and FPS overrides:

```python
frame = project.render_frame(0.5, resolution=(320, 180), backend="cpu")
project.render_frames(
    directory="small-sequence", resolution=(320, 180), fps=12, overwrite=True,
)
prepared = project.prepare(backend="cpu", resolution=(320, 180), fps=12)
```

A prepared runtime fixes its dimensions and frame grid for its lifetime.
Prepare again for different raster/FPS settings. Its video requests can select
an output format and range without rebuilding resources.

## Export GIF or animated WebP

```python
project.render("animation.gif", fps=15, start=0, end=0.5, overwrite=True)
project.render("animation.webp", fps=24, start=0, end=0.5, overwrite=True)
```

The extension selects the animation format for `render()` and prepared
`render_video()`. `frame.save("poster.webp")` still writes a static image.
Both animations loop indefinitely, include no audio stream, and retain the
original-time visual evaluation used by MP4. Audio analysis needed by visual
effects still runs. An inaudible audio clip that determines an automatic project
duration still needs media preflight.

GIF generates a local palette for every frame, with up to 255 visible colors
and one transparent entry. Alpha below 128 becomes transparent; other pixels
become opaque. Full frames replace previous content to avoid transparent motion
trails. GIF stores centisecond timestamps and accepts at most 100 FPS; readers
can impose their own minimum playback delay.

Animated WebP stores lossless full-canvas frames, including partial alpha, at
millisecond timing and at most 1000 FPS. RGB beneath fully transparent pixels
may be normalized by the encoder. Its canvas is limited to 16383 pixels per
side and the RIFF container to 4 GiB. GIF dimensions and frame delays also
respect their format limits. Rational frame boundaries are quantized to each
format's clock; durations can differ from the requested rational duration by
one clock tick. Extremely long individual delays are refused.

Vestra streams frames, palettes and WebP container chunks through bounded
buffers; it does not collect an entire animation in memory. Sequences likewise
release each exported frame before requesting the next. Output disk use still
grows with duration. Existing overwrite, cancellation, finalization and atomic
publication rules apply to animations. FFmpeg is resolved automatically with
the Python package; preflight reports a missing GIF or `libwebp` encoder clearly.

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
