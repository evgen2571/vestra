---
description: Know what validation, preflight, preparation, frame rendering and successful output publication establish.
---

# The rendering lifecycle

`project.render(...)` takes an authored project to a published video. A successful
validation report or a fully rendered last frame is an earlier milestone; the
output is complete only after encoding, finalization and publication succeed.

<figure class="diagram" markdown="span">
![A snapshot passes through semantic validation, environment preflight, preparation, frame and audio rendering with encoding, then finalization and publication before completion.](../assets/diagrams/lifecycle-render.svg)
<figcaption>Each step establishes something different. A valid description can still reference a missing file; a prepared runtime can render frames without publishing a video.</figcaption>
</figure>

<span id="validation-and-preparation"></span>

## Validate the description

`project.validate()` checks the canonical snapshot's structure, timing, ranges
and relationships. It does not probe media, test FFmpeg, inspect the destination
or initialize a graphics adapter. A valid report means the description satisfies
semantic rules; a missing media file can still fail the next step.

Snapshot creation itself can report authoring errors, such as an unavailable
font family. The pure validation boundary applies to the resulting canonical
project.

## Check the environment and prepare

Preflight checks the assets and media facts needed by the operation, FFmpeg,
output readiness and backend availability where applicable. CLI `ve validate`
includes its validation preflight target, so it can report environment problems
that Python `project.validate()` does not inspect.

Preparation validates and preflights, resolves media and audio analysis, compiles
the project plan and initializes the renderer. `project.prepare()` returns a
reusable `PreparedProject`; it does not encode or publish a video. The prepared
runtime is isolated from later authoring edits. Prepare again to include them.

## Evaluate and render

For each output frame, Vestra evaluates active layers, their local times,
keyframes, effects, signals and transitions, then renders that work. Audio is
prepared and processed for the output timeline. A single-frame render provides
RGBA pixels at a requested time; it does not exercise video encoding or output
publication.

For one inspection, `project.render_frame(seconds)` is convenient. It prepares
on every call. Reuse `project.prepare()` when inspecting several times or making
several outputs from the same snapshot. The [Python rendering guide](../guides/python/rendering-and-preparation.md)
shows both routes.

<span id="publication"></span>

## Finish and publish the output

Encoding and finalization must succeed before output publication. A rendering
progress fraction of `1.0` describes completed frame progress; finalizing the file
can still be underway. The terminal `completed` event and returned `RenderResult`
follow successful publication.

With `overwrite=False`, publication refuses an existing destination atomically,
including one created during the render. It uses a hard link from a sibling
temporary file, so a filesystem without hard-link support can fail publication.
`overwrite=True` deliberately replaces the destination using a rename.

Use the result or terminal event as the success signal. Rendering progress alone
does not establish that the output file is ready.

<span id="render-requests-and-events"></span>

## Failure and cancellation

A failure during validation, preparation, rendering or publication is not a
successful render. Cancellation is cooperative: `CancellationToken.cancel()`
requests it, and Python raises `CancelledError` rather than returning a result.
Progress callbacks receive typed lifecycle events. Callback exceptions and cleanup
errors have explicit behavior; the [Python runtime reference](../reference/python-api.md#native-runtime)
and [rendering guide](../guides/python/rendering-and-preparation.md#progress-and-cancellation)
cover these advanced integration details.
