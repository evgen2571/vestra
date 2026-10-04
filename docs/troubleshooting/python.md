# Python problems

## `import vestra` fails

For a published wheel, install with `uv add vestra` or `python -m pip install vestra`. For a source checkout, use `uv sync --locked --extra dev`. Test the selected environment with `python -c "import vestra"`. An import error from `_native` means the PyO3 extension was not built for that interpreter or cannot load its native dependencies. FFmpeg development-library failures during a source build are covered in [FFmpeg troubleshooting](ffmpeg.md).

## Runtime and type checker disagree

The compiled extension supplies runtime native classes; `python/vestra/_native.pyi` supplies their static contract. Confirm the imported package path and revision before treating a mismatch as an API bug. Rebuild the extension after native changes. High-level authoring names live in Python modules, so use the documented source constructor/property rather than inventing a canonical field on a `Source`.

## Validation passes but preparation or rendering fails

`Project.validate()` lowers the editing graph and performs semantic validation only. It does not check files, codecs, output paths or graphics adapters. Use `project.prepare()` or `project.render()` for those runtime checks; use `ve validate` when diagnosing a project file because it runs validation-target preflight. Relative media paths resolve from the project's `base_directory`, not necessarily the current shell directory.

## Progress callback or cancellation behavior is surprising

`render(..., on_progress=callback)` calls Python with typed v1 `RenderEvent`
values for `started`, `stage_changed`, `progress`, and the terminal outcome.
The callback replaces the built-in Auto terminal renderer. `completed` occurs only after output publication. If
the callback raises before terminal success, Vestra stops rendering and
re-raises the original Python exception. A callback error while receiving
`completed` cannot invalidate a published render. A native cleanup error may
be attached to a pre-terminal callback exception as `render_cleanup_error`.

## Explicit logging

Call `vestra.configure_logging(...)` when an application wants Vestra logs.
Importing the package and ordinary rendering are subscriber-neutral. The API
maps to the shared Rust observability implementation and supports human or
JSON Lines format, stderr, file, stderr-plus-file, levels, `filter=`, and the
advanced `RUST_LOG` override. Repeated global initialization raises a clear
runtime error. `output="stderr"` requires no `file`; `output="file"` and
`output="stderr_and_file"` require one. Supplying a file with
`output="stderr"` is rejected rather than silently changing the destination.
JSON tracing on stderr disables native terminal progress so
the stream remains valid JSON Lines; JSON tracing to a file leaves native
stderr progress available.

For ordinary cancellation, create a `CancellationToken`, pass it to `render`,
and call `cancel()` from another control path. Observed cancellation raises
`CancelledError`, which is a `RenderError`, rather than returning a successful
result. It is not a callback exception.
