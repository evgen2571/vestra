# Phase 8C-A audit

## Authoring contract

Phase 8C-A adds immutable `ScalarKeyframe`, `PointKeyframe`, and
`CropKeyframe` values. Tracks retain their Phase 8B identities and mutable
`base_value` property. `keyframe(*, time, value, interpolation)` appends an
immutable snapshot, `keyframes` returns a tuple in declaration order, and
`clear_keyframes()` clears the collection without replacing the track.

The canonical track form stays minimal when empty:

```json
{"base_value": 1.0}
```

When non-empty it adds an ordered `keyframes` array. Each keyframe has fields
in this order: `time`, `value`, `interpolation`. Named interpolation values are
`linear`, `hold`, `ease_in`, `ease_out`, and `ease_in_out`. A custom curve uses
`{"type":"cubic_bezier","x1":...,"y1":...,"x2":...,"y2":...}`.

Python rejects booleans, non-finite values, negative local times, wrong value
categories, opacity outside `0..=1`, non-positive scale, anchor values outside
unit space, non-finite Bézier controls, and Bézier x controls outside `0..=1`.
It leaves crop geometry, clip-duration range, duplicate times, and keyframe
order to native validation. Python never sorts, deduplicates, clamps, or
evaluates keyframes.

Keyframes use clip-local seconds. The base value applies before the first
keyframe. The interpolation on a destination keyframe controls the preceding
segment. A final keyframe holds after its timestamp. Crop tracks keep their
base value and keyframes while disabled, and serialize again when enabled.

## Files

Added `python/video_editor/authoring/animation.py`,
`python-tests/test_authoring_animation.py`,
`python-tests/test_authoring_animation_frames.py`, and
`python-tests/typing_failures/authoring_wrong_keyframe_value.py`.

Modified `tracks.py`, `values.py`, the authoring package exports, typing tests,
and the README.

## Verification

Passed on CPython 3.13.11 with Rust 1.96.1 and Cargo 1.96.1. FFmpeg and
FFprobe were both version 7.1.5:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace                 59 + 2 + 31 + 4 + 3 + 2 tests passed
cargo test -p video-editor --test public_sdk       4 passed
cargo test -p video-editor --test public_exports   2 passed
python3 crates/video-editor-cli/tests/schema_validation.py
python -m pytest python-tests             148 passed, 4 skipped
python -m mypy python/video_editor python-tests/test_typing.py
python -m mypy.stubtest --ignore-unused-allowlist \
  --allowlist python-tests/stubtest-private-hooks.txt video_editor
```

`maturin develop` passed. `maturin build` passed after freeing generated Cargo
build output; the first attempt failed because the filesystem had no free
space. The wheel contains `video_editor/authoring/animation.py` and
`video_editor/py.typed`. A clean CPython 3.13 virtual environment installed the
wheel from `/tmp`, outside the source tree, then created scalar, point, and
crop keyframes, built and validated a native project, rendered a CPU frame, and
rendered a ten-frame CPU MP4.

CPU frame coverage checks named interpolation and cubic Bézier timing.
CPU video coverage renders an animated image through FFmpeg, probes it with
FFprobe, confirms ten frames and one second, confirms it has no audio stream,
and confirms no temporary output remains.

Normal WGPU tests are adapter-gated. The full Python suite skipped four WGPU
tests because no compatible adapter was present. The strict command
`VIDEO_EDITOR_REQUIRE_WGPU=1 python -m pytest python-tests/test_render_wgpu.py
python-tests/test_wgpu_frames.py -q` failed all four selections with "WGPU
adapter request returned no compatible adapter". This is the expected
environmental failure, not runtime animation verification.

## Deferred work

Phase 8C-B effect authoring is not included. No effect, post-effect,
transition, preset, flash, video-asset, or multi-audio API was added.

## Final verdict

Phase 8C-A complete
