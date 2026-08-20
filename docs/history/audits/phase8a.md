# Phase 8A audit

Recorded on 2026-07-30.

## API and boundary

`video_editor.authoring` is a pure-Python package. Its public API is
`ProjectBuilder`, `AuthoringError`, `Color`, `Point`, `Crop`, `CubicBezier`,
`Interpolation`, `Sizing`, `BlendMode`, `Quality`, `DurationMode`, and the
recursive `JsonValue` type alias. These live in normal Python modules, not the
native stub. The top-level package deliberately does not re-export them.

`ProjectBuilder.to_dict()` returns a fresh owned schema-version 1 mapping.
`build()` calls `Project.from_dict(snapshot, base_directory=builder.base_directory)`
and returns the existing immutable native project. Native serialization may add
serde defaults, so builder and native dictionaries need not be textually equal.
The native dictionary remains the authoritative post-parse representation.

## Builder rules

The constructor is `ProjectBuilder(*, width, height, frame_rate, output_path,
duration=None, duration_mode=None, background="#000000", quality=Quality.BALANCED,
base_directory=".", name=None, metadata=None)`. It requires `output_path` and
uses the native immutable `FrameRate`; serialization reads its normalized
numerator and denominator and always writes `N/D`.

An authored duration selects explicit mode. Omitting duration selects automatic
mode. Explicit without duration and automatic with duration both fail locally.
The builder does not convert an empty automatic project into explicit mode.
Base directory accepts strings and string path-like objects, stays out of the
canonical mapping, and is supplied to every native build.

Optional `name` and top-level `metadata` are omitted, never emitted as null.
Within supplied metadata, `None` is canonical JSON null. Metadata accepts JSON
scalars, lists, and string-keyed mappings. The builder copies it at construction,
rejects unsupported values, bytes, tuples, sets, non-string keys, and non-finite
floats, and never retains caller containers. Every `to_dict()` call makes another
deep snapshot.

`Color` accepts only the canonical `#RRGGBB` and `#RRGGBBAA` forms and
normalizes hexadecimal digits to lowercase. `Point`, `Crop`, and `CubicBezier`
are frozen dataclasses with explicit canonical conversion. Bézier values emit
`{"type": "cubic_bezier", "x1", "y1", "x2", "y2"}`. Number inputs reject
booleans and non-finite values, but native validation still owns dimensions,
durations, crop ranges, and other cross-field rules.

## Validation, IDs, and ownership

`build()` performs native parsing without semantic validation. `validate()`
calls `Editor().validate(build())`; it returns the native `ValidationReport`,
does not preflight, and preserves native exceptions and diagnostics.

Private builder-local IDs separate namespace from generated prefix. Assets use
one `asset` namespace, so an explicit ID reserved for an image also blocks an
audio asset, while their counters generate `image-000001` and `audio-000001`
independently. Clips, transitions, and flashes each have their own namespaces.
Effects can use a scope key: clip effects are unique per owning clip and
post-effects have a separate collection scope. Counters are deterministic,
fixed-width, and never serialized. Explicit IDs reject empty and whitespace-only
strings, duplicates raise `AuthoringError`, and generation skips reservations.
A private identity-only owner token prepares Phase 8B handles to reject
cross-builder use. It is not public or serialized. `AuthoringError` is a
`ValueError` for local authoring-state failures only; native exceptions are not
wrapped.

## Corrected foundational contracts

`Sizing` is a frozen tagged value, not a string enum. Its exact canonical forms
are `{"mode": "original"}`, `{"mode": "fit"}`, `{"mode": "cover"}`,
`{"mode": "scale", "scale": 1.25}`, and `{"mode": "stretch", "width":
1280, "height": 720}`. `scale` requires a finite positive real value. `stretch`
requires positive integer width and height. All numeric inputs reject booleans.

`ProjectBuilder` has validated mutable properties for width, height, frame rate,
output path, background, quality, base directory, name, metadata, duration, and
duration mode. Assignments use the same local checks as construction. Assigning a
duration selects explicit mode; assigning `None` selects automatic mode and
removes the serialized duration. Setting automatic mode clears duration. Setting
explicit mode without a duration fails.

Output and base-directory values accept `str` and `os.PathLike[str]`, converted
through `os.fspath()`. Raw bytes and path-like values yielding bytes fail. Output
paths serialize as strings. Base directory remains native runtime context and is
not serialized.

The Python package contains mutable `ProjectBuilder` state and immutable
foundational values. Native `Project`, execution DTOs, and report DTOs remain
immutable.

## Runtime and package checks

Builder construction and native parsing need no FFmpeg. CPU rendering needs
FFmpeg, and output inspection needs FFprobe. WGPU is not needed for this
Phase 8A CPU smoke test. The focused test creates a background-only project
without a hand-written project dictionary, validates it, renders it through
CPU and FFmpeg, checks final publication, and uses FFprobe for its dimensions.

WGPU supports every valid schema-version 1 effect, blend mode, post-effect,
transition, and preset at plan compatibility time. This does not promise a live
adapter. `auto` can choose CPU when adapter discovery or WGPU preparation fails.
Explicit WGPU reports adapter, device, resource, shader, and runtime failures;
once prepared, it does not switch to CPU mid-operation.

Phase 8B remains deferred: assets, clips, handles, animation, effects,
transitions, presets, flashes, and audio authoring are intentionally absent.

## Verification

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
| `python3 crates/video-editor-cli/tests/schema_validation.py` | passed |
| `.venv/bin/python -m pytest python-tests` | 119 passed, 4 WGPU adapter-gated skips |
| `.venv/bin/python -m mypy python/video_editor python-tests/test_typing.py` | passed |
| `mypy.stubtest` with the repository allowlist | passed |
| `maturin develop` | passed |
| `maturin build` | passed after `cargo clean` reclaimed exhausted build-cache space |
| clean CPython wheel install outside the source tree | passed: import outside the source tree, `py.typed`, authoring serialization, build, validation, and CPU render |
| normal adapter-gated WGPU Python tests | skipped: no usable adapter |
| strict WGPU tests | skipped: no compatible adapter |

The tested interpreter was CPython 3.13.5 on Linux x86_64. Rust was 1.96.1 and
Maturin was 1.14.1. PyO3 was 0.29.0. FFmpeg 7.1.5 and FFprobe 7.1.5 were
available. WGPU compiled, while four adapter-gated Python tests skipped because
this container has no usable adapter. No claim is made for CPython 3.11 or 3.12,
another platform, or live WGPU rendering.

## Final verdict

Phase 8A complete.
