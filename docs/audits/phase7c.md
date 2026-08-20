# Phase 7C Python video-rendering audit

## Public API and scope

Phase 7C exposes synchronous CPU and adapter-gated WGPU video rendering through
`Editor.render()` and `PreparedProject.render_video()`. The binding depends
only on `video-editor`. It does not expose renderer state, FFmpeg processes,
temporary-file details, media internals, render plans, or WGPU handles.

`RenderRequest(output, *, backend=None, overwrite=False, preview=False)` is
the one-shot request. `PreparedVideoRenderRequest(output, *, overwrite=False)`
uses the backend selected during preparation. Both requests, cancellation
tokens, events, result DTOs, and reports are immutable Python objects.

## Callback contract

The binding checks `progress` while Python is attached. `None` and callables
are accepted. Any other value raises `TypeError("progress must be callable or
None")` before a prepared slot is acquired, one-shot preparation starts,
temporary output is created, or native rendering begins. Tests cover prepared
and one-shot calls, reset a private native-invocation counter, and confirm it
remains zero. They also confirm no output residue; the prepared object remains
usable.

Callbacks run synchronously on the Python thread that called the blocking
render method. The native operation releases the interpreter between callback
points. Callback return values, including `False`, strings, integers, and
arbitrary objects, are ignored. Only `started` and pre-publication `progress`
events are delivered. Native `completed` is intentionally filtered because a
successful method return is the Python completion signal.

Each event is an owned snapshot. `started` has frame zero, progress zero, and
the output path. Progress events have increasing frame numbers, a progress
value below one, and no output path. Warnings are immutable tuples and event
objects remain usable after the render returns.

The observer retains the first `PyErr`, disables later callbacks, requests
native cancellation, waits for cleanup, then re-raises the original exception.
Any cleanup error is attached as `render_cleanup_error` and remains secondary.
The adapter uses an explicit `let Some(callback)` branch, not `expect()` or
`unwrap()`, for user-supplied callback state. The Phase 7C binding paths have
no panic macro or value-unwrapping assumption.

Re-entering the same prepared object from a callback raises
`PreparedProjectBusyError`. If caught, the outer render continues. If uncaught,
that busy error is the original callback exception, the outer operation is
cancelled, no file is published, and the prepared object is restored.

## One-shot and prepared lifecycle coverage

One-shot tests cover callback failure on `started` and after progress,
already-cancelled tokens, cancellation from progress, overwrite refusal and
safe replacement. They check original exception preservation, no later
callbacks, no final output or temporary residue on failure, and `ONE_SHOT`
timing scope on success. `started` occurs after one-shot preparation and just
before rendering is submitted, so the one-shot path has no reusable native
prepared object after a failure.

Prepared concurrency tests use deterministic private barriers. A video active
inside detached native execution rejects both a second video call and a frame
call immediately. An active frame call rejects a video call immediately. Each
busy error has `kind == "busy"` with empty diagnostics and warnings. The active
operation completes after the barrier is released, the object remains usable,
and a different prepared object remains independent.

The detached-cancellation test arms `_test_arm_video_render`, which blocks
inside `PreparedSlot::video_operation` after it has acquired the prepared slot
and detached from Python, immediately before calling the native video render.
Another Python thread calls `token.cancel()` while that barrier is held. The
binding-level test therefore proves detached pre-entry cancellation: the SDK
observes an already-cancelled token on entry, returns `CancelledError`, leaves
no output, and restores the prepared object because submission had not started.
It is not a mid-render frame-loop polling test. Progress-triggered cancellation
covers post-submission cancellation in the binding, and Rust SDK lifecycle
tests cover cancellation polling separately. The hooks remain private to
`video_editor._native`, are absent from the public package and stubs, map
poisoned locks to `RuntimeError`, and are released in `finally` blocks.

## Failure and result mapping

A POSIX fake `ffmpeg` executable passes `-version` then exits during encoding.
The prepared CPU render fails at `FRAME_WRITE`, does not publish output,
removes the temporary output, and returns `RenderError` with tuple diagnostics
and warnings, `RenderTimings`, `RenderFailureContext`, and boolean cleanup
status. Because it fails after submission, subsequent frame use reports the
stable `VESTRA-PREPARED-INVALIDATED` diagnostic.

Prepared and one-shot success tests cover every Python `RenderResult` field:
editor and project paths, output path, dimensions, frame rate, duration,
frame count, visual clip count, audio and preview flags, elapsed values,
timing scope, timings, performance, requested and selected backend, encoder,
fallback, adapter, and warnings. Prepared results report `PREPARED_OPERATION`;
one-shot results report `ONE_SHOT`. Results and nested DTOs reject assignment.
They remain inspectable after editor, project, prepared object, and output file
are removed. The SDK reports duration in seconds and elapsed time in
milliseconds, so the Python API exposes no invented nanosecond fields.

`preview=True` on a one-shot `RenderRequest` is passed through shared
preparation into the compiled plan and returns `result.preview is True`.
Default results report `False`. A short audio project using
`examples/assets/tone.wav` returns `audio_present is True`; FFprobe reports
one audio and one video stream. A no-audio preview render reports false and
FFprobe reports only video.

## Typing and package checks

The stubs use `Callable[[RenderEvent], object] | None` for both callback
parameters and make token, event, result, and DTO properties read-only.
Positive typing accepts a callback returning `object`. Negative fixtures reject
integer and string callbacks, plus mutation of the exposed immutable fields.
Private runtime-only hooks remain absent from the public stubs. The narrow
`python-tests/stubtest-private-hooks.txt` allowlist records those private names
for the stubtest invocation; the installed stubtest version ignores private
runtime extras, so the command also uses `--ignore-unused-allowlist`.

## Final Phase 7C evidence

### No-callback fast path

`CallbackState::observe()` now returns before `Python::attach()` when
`progress=None`. It also returns before constructing a Python `RenderEvent`,
so native rendering stays detached. Prepared and one-shot real CPU video
renders each record zero callback-related attachments.

### Callback path and threading

The private atomic counter increments immediately before the callback adapter
attaches to Python. Real prepared and one-shot callbacks record one attachment
per forwarded `started` or `progress` event; native `completed` is filtered
before attachment. A callback failure disables the adapter, and the counter
does not increase for later native events. The worker thread test captures
only its ID and errors in the worker, joins it, and performs every callback
thread-identity assertion on the main test thread.

### WGPU and final verdict

WGPU code compiles. Normal adapter-gated tests skip without an adapter, while
strict tests fail rather than skip. No adapter-backed runtime render was run on
this host. The verification record below establishes that this is an
environmental validation item rather than an unresolved implementation defect.
Phase 7C is complete, and therefore Phase 7 is complete.

## Verification record

This work ran on Linux, CPython 3.13, with FFmpeg and FFprobe installed.

- Passed: `cargo fmt --all -- --check`
- Passed: `cargo check --workspace`
- Passed: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Passed: `cargo test --workspace`
- Passed: `cargo test -p video-editor`
- Passed: `cargo test -p video-editor-python`
- Passed: `cargo test -p video-editor --test public_sdk`
- Passed: `cargo test -p video-editor --test public_exports`
- Passed: `maturin develop`
- Passed: `.venv/bin/python -m pytest python-tests` (74 passed, 4 skipped)
- Passed: `.venv/bin/python -m mypy python/video_editor python-tests/test_typing.py`
- Passed: `.venv/bin/python -m mypy.stubtest --ignore-unused-allowlist --allowlist python-tests/stubtest-private-hooks.txt video_editor`
- Passed: `maturin build`
- Passed: fresh CPython 3.13 wheel smoke tests outside the source tree for CPU
  prepared and one-shot video, callback cleanup, cancellation cleanup,
  preview, audio, no-audio output, and packaged `py.typed`
- Passed, adapter-gated: normal WGPU tests skip when no adapter is available
- Failed as policy requires: `VIDEO_EDITOR_REQUIRE_WGPU=1 .venv/bin/python -m pytest python-tests/test_render_wgpu.py -q` reports two adapter-unavailable failures on this host

The strict WGPU failure verifies policy only. This machine has no compatible
adapter, so no adapter-backed WGPU runtime render is claimed. WGPU code still
compiles, and CPU rendering, callback handling, cleanup, lifecycle behavior,
typing, and package development installation are verified here.
