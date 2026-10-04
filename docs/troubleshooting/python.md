# Python problems

Start with the section matching the error you see. If you installed Vestra from
PyPI, the first checks apply to your installed package. Source-checkout checks
are collected at the end.

## `import vestra` fails

Check that Vestra is installed in the environment running your script.
For a uv project:

```bash
uv add vestra
uv run python -c "import vestra; print(vestra.__version__)"
```

For a pip environment, use the same Python interpreter for installation and
execution:

```bash
python -m pip install vestra
python -c "import vestra; print(vestra.__version__)"
```

An error mentioning `_native` means that Python cannot load Vestra's compiled
engine. Check that your Python version and platform are supported by the
[installation guide](../getting-started/installation.md). If you are working
from a checkout, see the source-build section below.

## My editor reports an error, but the script runs

Make sure your editor's type checker uses the same Python environment as your
script. Check which package the script imports:

```bash
uv run python -c "import vestra; print(vestra.__file__)"
# In a pip environment, use python instead of uv run python.
```

Compare the highlighted name or argument with the
[Python API reference](../reference/python-api.md). If the installed version,
editor environment and documented API agree but the warning persists, report
it with a small example and the type checker's message.

## Validation passes, but rendering fails

`project.validate()` checks project settings and layer relationships. It does
not check whether media files exist, codecs are available, the output path is
writable or a graphics adapter can be used. Those checks happen during
`project.prepare()` or `project.render()`.

Read the render error for the failing file or resource. Check `ffmpeg` and
`ffprobe` on `PATH`, and confirm that relative media paths are resolved from the
project's `base_directory`, which may differ from your shell's current directory.
See [FFmpeg troubleshooting](ffmpeg.md) and
[rendering troubleshooting](rendering.md) for the next checks.

The CLI's `ve validate` also checks resources needed by a project file, so it
performs more environment checks than Python's `project.validate()`.

## Progress callbacks raise an exception

`render(..., on_progress=callback)` sends `RenderEvent` v1 values for `started`,
`stage_changed`, `progress` and the terminal outcome. A callback replaces the
built-in terminal progress display. The `completed` event means the output
file has been finalized and published.

If a callback raises before rendering completes, Vestra stops rendering and
re-raises the original Python exception. Check the callback's traceback first.
An additional cleanup error may be attached as `render_cleanup_error`.
An exception from a callback handling `completed` does not invalidate the
already-published output.

## Cancelling a render raises `CancelledError`

This is the expected outcome of cancellation. Create a `CancellationToken`,
pass it to `render` and call `cancel()` from another control path to stop the
render. Handle `vestra.CancelledError` when your application needs to distinguish
cancellation from success. It is a `RenderError`, and is separate from exceptions
raised by a progress callback.

## Logs are missing or conflict with progress output

Call `vestra.configure_logging(...)` to enable Vestra logs. Importing Vestra or
rendering a project does not configure logging automatically. Configure it once;
repeated global initialization raises a runtime error.

Choose the destination consistently:

- `output="stderr"` does not accept a `file` argument.
- `output="file"` and `output="stderr_and_file"` require a `file` argument.

Logging supports readable text or JSON Lines, levels, `filter=` and the advanced
`RUST_LOG` override. JSON logs sent to stderr disable terminal progress so the
JSON stream stays readable by tools. JSON logs sent to a file leave terminal
progress available. See the [Python API reference](../reference/python-api.md)
for logging options.

## Problems in a source checkout

Rebuild the editable native package after changing Rust code or switching
interpreters. From the checkout's development environment:

```bash
just python-sync
uv run python -c "import vestra; print(vestra.__file__)"
```

An `_native` import failure can mean the extension was built for a different
interpreter or cannot load its native dependencies. For FFmpeg build errors,
see [FFmpeg troubleshooting](ffmpeg.md). Follow
[Contributing](../../CONTRIBUTING.md) and [Testing](../development/testing.md)
for environment setup and checks.

When changing the Python bindings, keep the compiled extension and its type
stubs in `python/vestra/_native.pyi` consistent. Rebuild before comparing runtime
behavior with type-checker results.
