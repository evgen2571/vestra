# Phase 7B audit

Recorded on 2026-07-30 in the workspace Linux environment with CPython 3.13.

## Public API

The Python package exports `PrepareOptions`, `PreparedProject`,
`PreparationReport`, `PreparationTimings`, `FrameRate`, `Frame`, `PixelFormat`,
`BackendKind`, `BackendFallback`, `AdapterInfo`, `AdapterDeviceType`, and
`GraphicsBackend`. `Editor.prepare(project, options=None)` returns a reusable
visual snapshot. `PreparedProject` provides `render_frame_number`,
`render_frame_ns`, and `render_frame_seconds`. Preparation, frame rendering,
and contention errors use `PreparationError`, `FrameRenderError`, and
`PreparedProjectBusyError` with `kind`, `diagnostics`, and `warnings`.

Runtime-contract tests verify the intended name and `video_editor._native`
module for every Phase 7B class and exception. No `Py*` wrapper aliases are
exported. `_native.pyi` declares the same names and read-only properties.

## Enum contract

`AdapterDeviceType` uses `DISCRETE_GPU`, `INTEGRATED_GPU`, `VIRTUAL_GPU`,
`CPU`, and `OTHER`. Their stable values are `discretegpu`, `integratedgpu`,
`virtualgpu`, `cpu`, and `other`.

`GraphicsBackend` uses `VULKAN`, `METAL`, `DX12`, `GL`, `BROWSER_WEBGPU`, and
`OTHER`. Their stable values are `vulkan`, `metal`, `dx12`, `gl`,
`browserwebgpu`, and `other`. Tests cover exact values, string conversion,
hashing, representation, and immutability.

## CPU behavior

A schema-valid image project with a missing asset passes deterministic
validation and fails CPU preparation with a structured `PreparationError`.
The test checks retained diagnostics and warnings plus diagnostic code,
category, severity, and message.

A deterministic 18-frame CPU project renders `[10, 2, 10, 0, 17, 1, 17, 5]`.
Repeated frames have identical bytes, decreasing access works, and later valid
access succeeds after an out-of-range request. The report snapshot includes
every public property. It stays unchanged after successful, repeated,
non-monotonic, out-of-range, and final-duration frame operations, and after
the source `Editor` and `Project` are dropped.

The pixel test writes a 2 by 2 RGBA PNG with red, half-transparent green,
partly transparent blue, and transparent white pixels. Rendering it on a blue
background checks the four packed RGBA8 pixels in row-major order:
`(255, 0, 0, 255)`, `(0, 128, 127, 255)`, `(0, 0, 255, 255)`, and
`(0, 0, 255, 255)`. This proves top-row-first rows, no row padding, channel
order, and straight-alpha source composition.

Prepared images are decoded snapshots. Replacing the PNG after preparation
does not alter an existing prepared object's frame bytes. A new prepared object
for the same project observes the replacement. This guarantee applies to
decoded visual assets only. It does not claim that external audio or media
opened during video rendering are frozen.

## Concurrency

The binding stores one prepared value in a ready or busy slot. A concurrent
same-object call returns `PreparedProjectBusyError` with `kind == "busy"` and
empty diagnostics and warnings while the active detached call remains blocked.
The active call completes, and the object remains reusable. A second prepared
object remains usable at the same time. Both worker tests capture exceptions,
release the native barrier in `finally`, join the worker, and assert that no
worker exception was hidden.

## WGPU

### Implementation and strict policy

WGPU code compiles. Normal Python WGPU tests skip only when adapter discovery
returns the recognized adapter-unavailable condition. Setting
`VIDEO_EDITOR_REQUIRE_WGPU=1` turns that condition into a test failure. This
run verified the strict-policy behavior: unavailable hardware cannot silently
pass as a skipped strict test.

### Environmental limitation

This container has no `/dev/dri` device and no usable Vulkan ICD manifest, so
WGPU cannot discover an adapter. The normal Python suite therefore passed with
two documented adapter-gated skips. The strict Python command failed, as
required, with `PreparationError` reporting no compatible adapter. This is an
environmental test limitation, not an implementation failure.

### Future production verification

Real WGPU execution remains unverified here. In a GPU-capable environment, the
existing tests must verify explicit WGPU selection and adapter metadata, owned
frame bytes, repeated and arbitrary access, prepared-object drop safety, and
CPU/WGPU metadata parity for dimensions, frame number, timestamp, pixel
format, and byte length. Production WGPU pixel-parity claims remain deferred
until that strict adapter-backed pass succeeds.

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
| `maturin develop` | passed |
| `.venv/bin/python -m pytest python-tests` | passed, 42 passed and 2 WGPU skips |
| `.venv/bin/python -m mypy python-tests/test_typing.py` | passed |
| `.venv/bin/python -m mypy.stubtest video_editor` | passed |
| `maturin build` and clean wheel install | passed |
| `VIDEO_EDITOR_REQUIRE_WGPU=1 .venv/bin/python -m pytest python-tests/test_wgpu_frames.py` | strict policy verified by expected adapter-unavailable failure; not a runtime pass |

The clean wheel check imported `video_editor`, verified `py.typed`, checked the
final enum members, prepared a CPU project, rendered non-monotonic frames, and
checked the exact 2 by 2 pixel bytes.

## Readiness

Phase 7B implementation is complete. CPU, API, typing, and package
verification passed. Normal WGPU adapter-gated behavior passed with a
documented skip, and strict-mode enforcement is verified.

The repository is ready for Phase 7C implementation. Adapter-backed Phase 7B
verification remains deferred to a GPU-capable environment. That limitation
blocks only a production WGPU parity claim, not Phase 7B completion or Phase 7C
implementation. No Phase 7C Python API is present.
