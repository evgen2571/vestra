# Phase 7A audit

Recorded on 2026-07-30. Phase 7A supplies the immutable Python foundation only:
project loading/conversion, validation, preflight, and inspection. It does not
expose prepared execution, frames, rendering, callbacks, WGPU, or FFmpeg
implementation objects.

## Runtime API and package boundary

`video-editor-python` depends directly on `video-editor`, PyO3 0.29,
pythonize, and serde_json only. It has no direct core, render, media, or CLI
dependency and adds no handwritten `unsafe`.

Every native public wrapper has an explicit PyO3 `name = "…"` and
`module = "video_editor._native"`. The public classes are `Project`, `Editor`,
`PreflightOptions`, `Diagnostic`, `ValidationReport`, `PreflightReport`,
`InspectionReport`, `InspectOutput`, `InspectAssets`, `InspectAudio`,
`BackendPreference`, `Category`, and `Severity`. Each is registered once;
there are no `PyProject`/`PyEditor`/other `Py*` module aliases. The top-level
`video_editor` package re-exports the supported names.

## Immutability and typing

Projects, options, reports, diagnostics, inspection DTOs, and enum wrappers
are frozen native classes. Diagnostic collections are fresh immutable tuples.
`_native.pyi` declares all their exposed values as read-only properties, not
writable instance variables. `python-tests/typing_failures/immutable_assignment.py`
is intentionally rejected by mypy for assigning `Diagnostic.code`,
`ValidationReport.is_valid`, and `PreflightOptions.overwrite`; its pytest
driver asserts all three deterministic diagnostics. Runtime tests also assert
assignment rejection.

Exception attributes intentionally follow normal Python exception mutability.
Every package-created `VideoEditorError` (including `ProjectError`) has a
`kind: str`, `diagnostics: tuple[Diagnostic, ...]`, and
`warnings: tuple[Diagnostic, ...]`. `ProjectError.kind` is always `"project"`.
Project failures retain SDK diagnostics when available (including missing files,
invalid JSON, and schema errors); otherwise `diagnostics` is `()`. Project
failures without warnings always expose `warnings == ()`. These are ordinary
exception attributes and may be reassigned; the tuple contents and their
`Diagnostic` values remain immutable. This is reflected in the stub and is not
claimed as frozen exception state.

## Errors and dictionary conversion

`Editor.inspect()` keeps `EditorError` intact while detached and converts it
only after Python is reattached. The resulting `VideoEditorError` carries the
native `kind` string plus immutable `diagnostics` and `warnings` tuples. A
missing-asset inspection test verifies that diagnostic code, category,
severity, message, pointer, related ID, and hint remain SDK-owned data where
present; the error is not a string-only conversion.

`Project.from_dict()` accepts exactly `collections.abc.Mapping[str, object]`.
It supports `dict`, `MappingProxyType`, `UserDict`, a deterministic custom
`Mapping`, and nested mappings, and normalizes them into an owned
`serde_json::Value` before detachment. Top-level non-mappings, non-string keys,
and unsupported nested values raise `TypeError`; recursive containers,
out-of-range integers, and non-finite floats raise `ValueError`; a valid
mapping that violates the project schema raises `ProjectError`. There is no
`str()` or `repr()` fallback. The owned conversion is verified after the
original custom mapping is dropped.

## GIL verification

Three underscore-prefixed native test hooks are deliberately hidden from the
supported top-level `video_editor` package and omitted from `_native.pyi`.
They are test infrastructure only. The worker enters `Python::detach`, marks a
Rust mutex/condition-variable barrier, and blocks; the main thread waits for
that explicit entry signal, executes Python work, releases the barrier, and
joins the worker. The test uses `finally` for release and joining and re-raises
any worker exception in the main thread. This proves entry, continued blocking,
Python execution during detached native work, release, and successful worker
return without payload-size, sleep, or elapsed-time claims. Poisoned mutex and
condition-variable operations become Python `RuntimeError`s rather than
panicking across the Python boundary.

Runtime-contract tests assert the exact public class name and
`video_editor._native` module for every Phase 7A class, in addition to proving
that no internal `Py*` aliases are exported.

## Verification and readiness

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo check --workspace` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` | passed |
| `cargo test -p video-editor` | passed |
| `cargo test -p video-editor-python` | passed |
| `cargo test -p video-editor --test public_sdk` | passed |
| `cargo test -p video-editor --test public_exports` | passed |
| `maturin develop` | passed |
| `.venv/bin/python -m pytest python-tests` | passed: 28 tests |
| `.venv/bin/python -m mypy python-tests` | passed |
| `.venv/bin/python -m mypy.stubtest --ignore-missing-stub video_editor` | passed; private native test hooks intentionally have no public stub |
| `maturin build` | passed |
| clean CPython virtual-environment wheel install and smoke test | passed: import, `py.typed`, exact names/modules, `ProjectError` context, project conversion/loading, validation, and inspection |

The available environment is Linux with CPython 3.13.5 only; no claim is made
for other Python versions, platforms, WGPU adapters, FFmpeg-dependent
rendering, or rendering APIs. The JSON/path conversion boundary lives in its
own `conversion.rs` module; future prepared-state wrappers should be added in
focused modules rather than extending the Phase 7A conversion layer.

**Ready for Phase 7B.**
