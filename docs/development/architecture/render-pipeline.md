# Render pipeline

`Editor` prepares a project once so callers can inspect individual frames or
render several videos from the same runtime state. One-shot `Editor::render`
uses the same preparation coordinator, then runs the video path.

<figure class="diagram" markdown="span">
![Validation, preflight and resource preparation produce a reusable PreparedProject. Individual-frame requests return RGBA pixels. Video requests render frames and audio, finalize the encoder, publish the file and return RenderResult.](../../assets/diagrams/prepared-execution.svg)
<figcaption>Repeat either operation from prepared state. Only the video path encodes and publishes output.</figcaption>
</figure>

## Prepared state and frame ownership

Preparation starts with semantic validation and operation-specific preflight.
`Editor::prepare` checks the backend and input resources without checking an
output destination or requiring an encoder. One-shot rendering also checks the
destination, overwrite policy and encoder availability. Preflight errors return
as diagnostics before resource preparation begins.

`PreparedProject` retains the compiled plan, prepared resources, selected backend
and a stable `PreparationReport` with adapter data, warnings and preparation
timings. Keep source files available and unchanged: video decodes on demand,
and audio reopens for each encoded video even when visual analysis is already
prepared. The [Python rendering guide](../../guides/python/rendering-and-preparation.md#prepare-once-for-several-frames)
shows reuse across frames and outputs.

Each frame evaluates the plan at its checked timeline time and submits work to
the selected backend. For video, the SDK borrows the owned frame's RGBA bytes
for the media sink write and retains the frame until any scoped writer joins.
The video operation also executes audio and emits `RenderEvent` values.

Rust rendering requires exclusive mutable access to prepared state. The Python
wrapper guards it with a mutex and reports `PreparedProjectBusyError` on
concurrent use. `CancellationToken` is cooperative; cancellation observed before
publication cannot yield a successful result.

## Layer presentation order

Each layer passes through these stages in order:

1. Source sizing/crop or child precomposition.
2. Pre-transform Motion Tile, then the final transform.
3. Post-transform effects, ordered mask coverage and track matte.
4. Final opacity and blend/composite.

Group masks apply to the completed group result; child masks remain part of
their child layers. The [effect pipeline](effect-pipeline.md#ordering-and-spaces)
explains effect stages and coordinate spaces.

## Runtime output settings

One-shot rendering and reusable preparation accept runtime raster dimensions,
FPS and encoder quality. FPS and quality are applied to an operation-local
project copy before validation and compilation, preserving authored project data.
The compiled canvas retains authored dimensions separately from output dimensions.
Frame evaluation resolves local coordinates and motion against the authored
canvas, then adapts presentation, masks and pixel-valued effect parameters before
either backend rasterizes. Source rasters and video decoding keep their original
resolution; composition and effect surfaces use the requested output density.

Different aspect ratios add a centered fitted composition at output density.
Global effects still run on the final canvas, including its background borders.
Existing kernel/cell limits remain enforced; upscaling beyond a physical effect
limit returns a diagnostic. The default-size path avoids this adaptation.

GIF and animated WebP use the same ordered RGBA frame loop and media-owned sink
as dynamic MP4. GIF uses per-frame palettes; WebP streams independent lossless
frames into a bounded RIFF mux. Both omit encoded audio, retain required visual
analysis, and use the normal temporary-output, abort and publication lifecycle.

## Completion, failure and reporting

Writing frames, finalizing the temporary encoder output and publishing the
destination are separate failure boundaries. A successful frame loop does not
establish that the file is ready. `RenderResult` and the terminal `completed`
event follow successful publication.

`RenderFailureContext` identifies the failed stage and may retain temporary and
destination paths. Reports record requested and selected backend, fallback,
adapter metadata, timings and performance counters. Use the selected backend
as evidence of what ran.

Use [media verification](../media-pipeline.md#verify-encoded-output) to check
encoded output and [render lifecycle](../../concepts/rendering-lifecycle.md)
for the public event and cancellation contract.
