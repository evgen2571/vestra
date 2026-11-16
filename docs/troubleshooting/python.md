# Python problems

Start with the error you see. The first sections apply to an installed package;
[source-checkout checks](#problems-in-a-source-checkout) are separate. Run each
check with the same Python environment that runs your script.

## `import vestra` fails

**Check.** Read the full traceback and compare your interpreter/platform with
[wheel support](../getting-started/installation.md#supported-python-versions-and-platforms).
`ModuleNotFoundError: vestra` usually means the active environment does not
contain the package. An error mentioning `_native` means the compiled engine
cannot be found or loaded.

**Fix.** Install in the environment that runs your script. For a uv project:

```bash
uv add vestra
```

For a pip environment, use `python -m pip install vestra`. If no published wheel
matches the interpreter/platform, use a supported interpreter or follow the
[advanced source-build route](../development/source-builds.md). Rebuilding a
checkout is a separate case below.

**Confirm.** Check import and version with that interpreter:

```bash
uv run python -c "import vestra; print(vestra.__version__); print(vestra.__file__)"
```

In a pip environment, use `python` instead of `uv run python`. If a supported
wheel still fails to load, keep the traceback, OS/architecture, Python version
and package version for a report.

## My editor reports an error, but the script runs

**Check.** Compare the editor's selected interpreter with the script's package:

```bash
uv run python -c "import vestra; print(vestra.__file__)"
```

Use `python` in a pip environment. Compare the highlighted name/argument and
installed version with the [Python API reference](../reference/python-api.md).

**Fix.** Select the same environment in the editor. For a source checkout,
rebuild the native extension before comparing runtime behavior with stubs.

**Confirm.** Rerun the script and the type checker. If they still disagree,
report the smallest example, installed version and exact type-checker message.

## Validation passes, but rendering fails

**Check.** `project.validate()` checks project settings and layer relationships.
It does not check media existence, codecs, writable outputs or graphics adapters.
Those checks happen during `prepare()` or `render()`. Read the error for the
failing file/resource and check relative paths against `project.base_directory`,
which may differ from your shell's current directory.

**Fix.** Correct the named path, output or backend problem. For FFmpeg failures,
check the executable path in the diagnostic and any `IMAGEIO_FFMPEG_EXE`
override. Python selects its installed executable automatically; it does not
require system FFmpeg on `PATH`. See [FFmpeg checks](ffmpeg.md) and
[rendering checks](rendering.md).

**Confirm.** Rerun the operation that failed. Semantic validation passing again
does not verify resource readiness. The CLI's `ve validate` includes preflight,
so it performs more environment checks than Python `project.validate()`.

## Progress callbacks raise an exception

**Check.** Read the callback's traceback first. `on_progress=callback` receives
`RenderEvent` v1 values for `started`, `stage_changed`, `progress` and the terminal
outcome, and replaces built-in terminal progress. Inspect
`error.render_cleanup_error` if an additional cleanup error was attached.

**Fix.** Correct the callback. If it raises before completion, Vestra stops
rendering and re-raises the original Python exception. A callback exception
handling `completed` does not invalidate the already-published output.

**Confirm.** Rerun with the corrected callback. Wait for the normal render
result; a terminal `completed` event means finalization and publication have
already succeeded. Keep the traceback if the exception persists.

## Cancelling a render raises `CancelledError`

**Check.** This is the expected cancellation outcome. Check which control path
called `cancel()` on the `CancellationToken` passed to `render()`.

**Fix.** Handle `vestra.CancelledError` when your application needs to distinguish
cancellation from success. It is a `RenderError`, separate from an exception
raised by a progress callback. If cancellation was unintended, correct the token
or application control path.

**Confirm.** An intentional cancellation should raise that exception rather
than return a success result. Run again without cancelling to verify normal
completion.

## Logs are missing or conflict with progress output

**Check.** Importing Vestra or rendering does not configure logging automatically.
`vestra.configure_logging(...)` initializes it once; repeating global
initialization raises a runtime error. Inspect the level/filter and destination:

| Destination | File argument | Progress behavior |
| --- | --- | --- |
| `output="stderr"` | Rejected. | JSON logging disables terminal progress on stderr. |
| `output="file"` | Required. | Terminal progress remains available. |
| `output="stderr_and_file"` | Required. | JSON logging disables terminal progress on stderr. |

**Fix.** Configure logging once with a matching destination/file combination.
Choose readable text or JSON Lines and a level/filter that includes the records
you need. An advanced `RUST_LOG` override can change filtering; see
[logging options](../reference/python-api.md#logging).

**Confirm.** Run the failing operation again and check the selected destination.
Keep JSON logs and render events as separate streams; enabling one does not
enable the other.

## Problems in a source checkout

**Check.** An `_native` load failure can mean the extension was built for another
interpreter or cannot load native dependencies. Switching interpreters or
changing Rust code requires rebuilding the editable native package.

**Fix.** From the checkout's configured development environment:

```bash
just python-sync
```

For prerequisites, use [source builds](../development/source-builds.md); for
native media build failures, see [FFmpeg troubleshooting](ffmpeg.md#build-cannot-find-ffmpeg).
Binding changes must keep the compiled extension, package exports and
`python/vestra/_native.pyi` consistent.

**Confirm.** Check that the imported package is the intended checkout:

```bash
uv run python -c "import vestra; print(vestra.__file__)"
```

Then rerun the failing script or focused check from [Testing](../development/testing.md).

## Report a problem that remains

Include the smallest script, complete traceback/type-checker message, exact run
command, Vestra and Python versions, OS/architecture, imported package path and
whether you used a wheel or a source checkout. For a checkout, include the commit
and build command. For render failures, add the
[render evidence](rendering.md#report-an-unresolved-render-failure), including
selected backend and any FFmpeg override. Remove private data before sharing.
