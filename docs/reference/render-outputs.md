---
description: Current contracts for render formats, output settings, frame selection, alpha and publication.
---

# Render outputs

Vestra exports MP4, GIF, animated WebP, individual frames and image sequences.
All use the same evaluated composition and selected CPU or WGPU backend.
For Python examples, see [rendering and preparation](../guides/python/rendering-and-preparation.md).

## Formats

| Output | Selection | Encoding and alpha |
| --- | --- | --- |
| MP4 | `.mp4` render destination | H.264 with optional AAC audio. |
| GIF | `.gif` render destination | Per-frame palette; binary alpha; silent, infinite loop. |
| Animated WebP | `.webp` render destination | Lossless frames with partial alpha; silent, infinite loop. |
| PNG | `.png` still destination | Lossless RGBA. |
| JPEG | `.jpg` or `.jpeg` still destination | Transparent pixels composited over black. |
| Static WebP | `.webp` still destination | Lossless RGBA. |

`render()` and prepared `render_video()` select a video or animation format by
destination extension. `Frame.save()`, `render_frame()` and `render_frames()`
write still images. Still-image encoders are built in; MP4 and animation encoding
use the FFmpeg executable. Python selects it automatically through `imageio-ffmpeg`;
Rust and the CLI default to `ffmpeg` on PATH.

Canonical schema-v1 output paths remain MP4 paths. Alternate formats are selected
through runtime output requests, without changing the canonical project format.

## Output settings

Runtime overrides leave the authored project unchanged. One-shot rendering and
preparation accept resolution, frame rate and quality; frame/sequence helpers
accept resolution and frame rate. Prepared state fixes these settings for its
lifetime. Prepared video requests can change the destination, format and range.

| Setting | Accepted values and behavior |
| --- | --- |
| Resolution | Even integer width and height in `2..=8192`, within resource limits. Python: `resolution=(width, height)`. |
| FPS | Positive integer, rational `(numerator, denominator)` pair or `FrameRate` in high-level Python. Native Python options use `FrameRate`. |
| Preview | Caps the longest side at 640 pixels and FPS at 15; selects preview quality. Never increases dimensions or FPS. Explicit settings override their respective defaults. |

### MP4 quality

| Python `Quality` | H.264 CRF | Encoder speed preset |
| --- | --- | --- |
| `PREVIEW` | 30 | `ultrafast` |
| `BALANCED` | 23 | `veryfast` |
| `HIGH` | 18 | `medium` |

Quality controls apply to MP4. GIF quantization and WebP lossless encoding use
the fixed policies below.

### Coordinates and resource limits

Resolution overrides change composition surfaces and raster work. Source sizing,
positions, rotations, groups, masks and pixel-valued effects retain the authored
coordinate space. A different aspect ratio uses centered uniform fit; uncovered
pixels use the project background. Global effects run on the final canvas,
including these borders.

Small kernels and stylization cells round to the output pixel grid. Upscaling
can exceed an effect's physical kernel or cell limits; it fails with a render
diagnostic rather than clipping the effect. A changed aspect ratio with both a
partially transparent background and a non-normal root blend mode is refused.
Use matching aspect ratios or an opaque/fully transparent background for that
combination.

Source decoding and prepared source rasters retain native resolution. Reducing
output resolution therefore saves less time when decoding dominates.

## Ranges and frame boundaries

Video and continuous image sequences select original project sample times in
`[start, end)`. Both boundaries round up to frame indices at the selected
output's rational FPS. An omitted start means zero; an omitted end uses project
duration. A range must satisfy `0 <= start < end <= duration` and contain at
least one sampled frame. Reversed, out-of-timeline and zero-sample intervals
produce `VESTRA-RENDER-RANGE`.

At 10 FPS, `[0.25, 0.68)` selects project frames 3–6. The video contains four
frames at timestamps 0, 0.1, 0.2 and 0.3 seconds, lasting 0.4 seconds. Effects,
transitions, source selection and audio-driven visuals keep their original
project time. Earlier output frames are not composed. Changing output FPS
changes the frame grid used for selection and evaluation.

For MP4, audio is mixed with the original effects and automation, then trimmed
from the first selected frame's project time and rebased to zero. Audio
boundaries round to the nearest 48 kHz mixer sample. A final frame can extend
beyond authored duration by less than one frame; the corresponding audio tail
is silence. Progress counts selected frames; failure frame indices refer to
the original project.

## Frames and arrays

Single-frame Python helpers require exactly one of `seconds` or `frame_number`.
Times select the containing frame; frame indices are zero-based. A `Frame`
records the selected project frame number and timestamp.

Pixels are RGBA8 with straight (unpremultiplied) alpha, top row first.
`Frame.to_bytes()` works without NumPy. `Frame.to_numpy()` and `Project.get_frame()`
require the `numpy` extra and return writable contiguous `uint8` arrays shaped
`(height, width, 4)`, with byte strides `(width * 4, 4, 1)`.

Each array is an independent copy that owns its memory. It remains valid after
the frame or prepared project is released and cannot alter the renderer.

## Image sequences

`render_frames()` accepts one of `times`, `indices`, or a continuous range.
The default is the whole timeline. Explicit selections preserve order and
duplicates; an explicitly empty list writes nothing and returns zero. Continuous
ranges use the boundary rules above.

Files are named `frame_000000.ext`, `frame_000001.ext`, and so on. The number
refers to selection order, not the original project frame. Existing numbered
destinations are checked before writing. Each image is published independently,
so a later encoding or I/O failure leaves earlier completed images available.

Only one frame is encoded at a time and the return value is a count. Vestra
does not retain a growing list of frames or output paths.

## Animation formats

GIF and animated WebP loop indefinitely and omit audio encoding. Required audio
analysis still runs for visual effects; audio that determines automatic project
duration still needs media preflight.

| Property | GIF | Animated WebP |
| --- | --- | --- |
| Color | Local palette per frame, up to 255 visible colors and one transparent entry. | Lossless full-canvas frames with partial alpha; RGB beneath fully transparent pixels may be normalized. |
| Transparency | Alpha below 128 becomes transparent; other pixels become opaque. | Partial alpha retained. |
| Timing | Centiseconds; at most 100 FPS. Readers may impose a minimum delay. | Milliseconds; at most 1000 FPS. |
| Format limits | Dimensions and frame delays must fit GIF fields. | Canvas at most 16383 pixels per side; RIFF container at most 4 GiB. Runtime resolution limits also apply. |

Full frames replace previous content to avoid transparent motion trails.
Rational frame boundaries are quantized to the format's clock; duration can
differ from the requested rational duration by one clock tick. Extremely long
individual delays are refused.

Frames, palettes and WebP container chunks stream through bounded buffers.
Vestra does not collect the whole animation in memory, though disk use grows
with duration. Preflight reports a missing GIF or `libwebp` encoder.

## Publication and failures

Existing outputs require explicit overwrite permission. Each output file is
encoded to a temporary target and published after encoding succeeds. Video and
animation output also require successful container finalization. Failures retain
the original destination where possible; still-image encoding or write failures
preserve an existing destination.

Frame progress reaching `1.0` precedes finalization and publication. A successful
`RenderResult` and terminal `completed` event follow publication. Cancellation
and cleanup use the same lifecycle across MP4 and animation output. See
[rendering lifecycle](../concepts/rendering-lifecycle.md) and
[diagnostics](diagnostics.md) for events, failure stages and error codes.
