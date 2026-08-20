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
creates a canonical snapshot, validates the project and its resources, prepares
the render plan, and sends the resulting frames and audio to the output
encoder.

## Validation and preparation

Validation checks project structure, semantic constraints, assets, and output
requirements. Preparation resolves the work needed before frame evaluation,
including media and audio analysis and renderer setup. A preparation failure
does not mean that a frame was rendered successfully.

Advanced Python code can call `project.validate()` or `project.prepare()`
before rendering. The immutable runtime SDK exposes the same split through
`Editor`, `ProjectSnapshot`, `PrepareOptions`, and `PreparedProject`.

## Render requests and events

A render request supplies an output path, overwrite policy, preview choice, and
backend preference. A progress callback receives events for the start of the
render, frame progress, and completion. The result includes the output path,
timing data, requested backend, selected backend, and adapter information when
the backend provides it.

Cancellation is cooperative. Pass a `CancellationToken` to the native or
Python runtime operation and request cancellation from the controlling code.
The result is then a cancellation outcome, not a completed publication.

## Publication

Encoding and output publication happen after preparation and frame evaluation.
Use `overwrite=True` or the corresponding CLI flag when replacing an existing
file is intentional. A successful render result describes the published output;
it is not just an indication that preparation succeeded.
