# Python problems

## `import vestra` fails

Vestra is source-oriented in this repository. Build/install it with the documented locked environment, for example `uv sync --locked --extra dev`, then test the exact environment with `uv run python -c "import vestra"`. An import error from `_native` means the PyO3 extension was not built for that interpreter or cannot load its native dependencies. FFmpeg development-library failures during the build are covered in [FFmpeg troubleshooting](ffmpeg.md).

## Runtime and type checker disagree

The compiled extension supplies runtime native classes; `python/vestra/_native.pyi` supplies their static contract. Confirm the imported package path and revision before treating a mismatch as an API bug. Rebuild the extension after native changes. High-level authoring names live in Python modules, so use the documented source constructor/property rather than inventing a canonical field on a `Source`.

## Validation passes but preparation or rendering fails

`Project.validate()` lowers the editing graph and performs semantic validation only. It does not check files, codecs, output paths or graphics adapters. Use `project.prepare()` or `project.render()` for those runtime checks; use `ve validate` when diagnosing a project file because it runs validation-target preflight. Relative media paths resolve from the project's `base_directory`, not necessarily the current shell directory.

## Progress callback or cancellation behavior is surprising

`render(..., progress=callback)` calls Python with `started` and `progress`
`RenderEvent` values. It does not deliver native `completed`, which is emitted
only after output publication. If the callback raises, Vestra stops rendering
and re-raises the original Python exception. No `RenderResult` is returned. A
native cleanup error may be attached to that exception as
`render_cleanup_error`.

For ordinary cancellation, create a `CancellationToken`, pass it to `render`,
and call `cancel()` from another control path. Observed cancellation raises
`CancelledError`, which is a `RenderError`, rather than returning a successful
result. It is not a callback exception.
