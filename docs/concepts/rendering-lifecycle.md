# The rendering lifecycle

At a high level, Vestra takes an authored project through a small number of
stages:

```text
author
  → snapshot and validate
  → prepare
  → select a backend
  → evaluate frames and audio
  → encode and publish output
  → return a result
```

`Project.render(...)` performs this flow for a normal one-shot render. It
creates a canonical snapshot, runs semantic validation and environment
preflight, prepares the render plan, and sends the resulting frames and audio
to the output encoder.

## Validation and preparation

`Editor.validate(project)` checks canonical project structure and semantic
constraints. It does not read assets, run FFmpeg, inspect the output path, or
initialize a renderer. `Editor.preflight(project, options)` performs the
environment-dependent checks for a selected operation, including assets/media,
FFmpeg, output readiness, and backend availability where applicable.

Preparation runs semantic validation and preflight before resolving media and
audio analysis, compiling the plan, and setting up the renderer. A preparation
failure does not mean that a frame was rendered successfully. The CLI
`ve validate` command uses the validation preflight target, so it can report
environment failures that SDK/Python `project.validate()` does not inspect.

Advanced Python code can call `project.validate()` or `project.prepare()`
before rendering. The immutable runtime SDK exposes the same split through
`Editor`, `ProjectSnapshot`, `PrepareOptions`, and `PreparedProject`.

## Render requests and events

A render request supplies an output path, overwrite policy, preview choice, and
backend preference. Native Rust render observers and Python progress callbacks
receive the typed lifecycle `started`, `stage_changed`, `progress`, and one
terminal outcome. `Completed` is emitted only after encoder finalization and
successful output publication. Rendering fraction is frame progress only and
may reach `1.0` before encoding/finalizing complete. A callback error before
terminal success may abort the operation; a callback error while receiving
`Completed` cannot invalidate a published render. The result includes the
output path, timing data, requested backend, selected backend, and adapter
information when the backend provides it.

Cancellation is cooperative. Pass a `CancellationToken` to the native or
Python runtime operation and request cancellation from the controlling code.
Rust returns a render error marked as cancelled. Python raises
`vestra.CancelledError`; cancellation is not a successful `RenderResult`.
That is separate from a Python callback exception. Vestra stops the render,
then re-raises the original callback exception. If native cleanup reports an
error, Vestra attaches it to that exception as `render_cleanup_error`.

## Publication

Encoding and output publication happen after preparation and frame evaluation.
Use `overwrite=True` or the corresponding CLI flag when replacing an existing
file is intentional. A successful render result describes the published output;
it is not just an indication that preparation succeeded.
