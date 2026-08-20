# Phase 9A: schema-v2 audio timeline

The active canonical loader accepts schema version 2 only; it does not migrate
schema v1. `Project.audio` is optional `AudioTimeline { tracks }`. A track has
`id`, `mute`, linear `gain`, and ordered clips. A clip has `id`, `asset`,
`start`, `trim_start`, optional `trim_end`, `mute`, linear `gain`, `fade_in`,
and `fade_out`. Omitted audio and omitted `trim_end` serialize by omission.

Track IDs are timeline-wide unique and clip IDs are unique across the whole
timeline. Python allocates `audio-track-000001` and `audio-clip-000001` in
builder-local transactional namespaces. Order is declaration order, never ID
or map order. Gain is finite and non-negative; values above one are valid.
Fades are linear and must fit the selected source duration during preflight.
Audio overlap is legal.

Audio clips structurally extend automatic duration regardless of mute, gain,
or `output.audio`; explicit duration retains truncation warnings. Every clip
counts as asset use and every referenced source is probed once per asset.
Limits are 256 tracks and 4096 total clips. The core logical `AudioMixPlan`
retains source paths, trims, selected duration, timing, fades, gain, mute, and
declaration order without FFmpeg syntax or millisecond quantization.

## Finalization: resolved timeline limits and core contracts

Pure core validation retains its early visual-only resource checks. After SDK
preflight probes audio sources and resolves the final automatic duration,
`project::validation::preflight` submits the resolved duration and checked
rational frame count to the shared core timeline-limit authority. This closes
the late-audio placement gap: both maximum duration and maximum frame count
are inclusive at their configured boundary and reject only values above it.
Both use `MVP-LIMIT-TIMELINE`, with a deterministic message and path that name
the violated duration or frame-count limit. The checked timeline conversion
continues to report `MVP-TIMELINE-OVERFLOW` when a value cannot be represented.

Direct Rust tests now cover track and timeline-global clip IDs, asset kind and
existence, finite non-negative track/clip gain (including gain above one),
legal same-track and cross-track overlap, mute/output-independent validation,
and inclusive audio track/clip resource boundaries. Preflight tests place the
short deterministic tone fixture at the configured duration and frame limits,
including below, exact, and above-boundary cases. Duration tests lock the
structural invariant for clip mute, track mute, zero clip/track gain, and
`output.audio = false`; empty tracks do not extend automatic duration while a
later valid clip does.

`AudioMixPlan` tests directly verify declared track order (`music`, `ambience`,
`sfx`), declared clip order (`clip-b`, `clip-a`, `clip-c`), all tracks/clips,
overlap preservation, and raw track/clip mute, gain, source/path, start,
trims, selected duration, and fades. The plan remains backend-neutral.

`output.audio` only controls mux eligibility. Phase 9A derives the existing
single-input media settings for exactly one audible clip. More than one
audible clip fails with `MVP-AUDIO-MIX-UNSUPPORTED` before output publication;
video-only output remains available when `output.audio` is false. Full
multi-input FFmpeg mixing, normalization, sample-accurate placement, and
quantitative audio tests are deferred to Phase 9B.

## Verification

Finalization verification on Linux / Python 3.13.5:

- `cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo clippy
  --workspace --all-targets --all-features -- -D warnings` passed.
- `cargo test --workspace` passed from a clean Cargo cache. Focused package
  gates also passed: `cargo test -p video-editor-core` (40 tests), `cargo test
  -p video-editor-media` (14 tests), `cargo test -p video-editor`, `cargo test
  -p video-editor --test public_sdk` (31 tests), and `cargo test -p
  video-editor --test public_exports` (2 tests).
- `python3 crates/video-editor-cli/tests/schema_validation.py` passed.
- `.venv/bin/maturin develop`, `.venv/bin/python -m pytest python-tests` (236
  passed, 4 adapter-gated skips), `.venv/bin/python -m mypy python/video_editor
  python-tests/test_typing.py`, and `.venv/bin/python -m mypy.stubtest
  --ignore-unused-allowlist --allowlist python-tests/stubtest-private-hooks.txt
  video_editor` passed.
- `maturin build` passed. An isolated clean-wheel environment imported
  `video_editor` and `video_editor.authoring`, constructed schema-v2 audio,
  built, validated, inspected, and CPU-rendered one audio clip successfully.
- FFmpeg and FFprobe 7.1.5 were available. The four Python WGPU tests were
  adapter-gated skips; no audio work enters the WGPU path.

The initial workspace test attempts were storage-constrained. `cargo clean`
removed 8.1 GiB of regenerable Cargo artifacts, restoring enough capacity for
the clean-cache workspace and package gates above to pass.
