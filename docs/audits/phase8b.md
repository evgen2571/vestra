# Phase 8B audit

Recorded on 2026-07-30.

## Public API

`video_editor.authoring` now exports `ImageAsset`, `AudioAsset`, `ImageClip`,
`SolidColorClip`, `AudioTrack`, `Transform`, `ScalarTrack`, `PointTrack`, and
`CropTrack`. `ProjectBuilder` adds `add_image_asset`, `add_audio_asset`,
`add_image_clip`, `add_solid_color_clip`, `set_audio`, and `clear_audio`.
`assets`, `clips`, and `audio` return immutable tuple snapshots or the current
audio node.

Asset handles are immutable and compare by builder identity, category, and
canonical ID. They keep their ID and source after `build()`. A same-category
handle from a different builder raises `AuthoringError`; a wrong category raises
`TypeError`. Handles do not carry owner state into project data.

## Canonical model confirmed from source

The Rust `Asset` mapping is `{id, type, source}` where `type` is `image` or
`audio`. Native validation stores both in one ID map, so Python reserves both
through the `asset` namespace. Images use clip source
`{"type": "image", "asset": id}`. Solids use
`{"type": "solid_color", "colour": "#rrggbb"}`.

Each clip requires `id`, `source`, `start`, `duration`, `layer`, and `opacity`.
`visible` defaults to true; `sizing`, `crop`, and `transform` are optional in
the model. The schema requires `transform` for images and forbids it for solid
colours. The renderer's transform identity is position and anchor `(0.5, 0.5)`,
scale `(1.0, 1.0)`, rotation `0.0`. Omitted sizing is native `original`.
Omitted crop has no crop. Every authored static track writes only
`{"base_value": value}`. Timing is project-timeline seconds. Track keyframe
times, when Phase 8C adds them, are clip-local.

The draw key uses layer, start, then clip ID, rather than declaration order.
Python retains declaration order in `visual.clips` and uses deterministic IDs.

The optional top-level audio object contains `asset`, `timeline_start`,
`trim_start`, `volume`, and optional `trim_end`, plus serde-defaulted
`fade_in`, `fade_out`, and `mute`. `set_audio()` replaces the current track.
It synchronizes `output.audio`; clearing the track removes top-level `audio`.
Muted tracks remain serialized, but native duration and render behavior ignore
them. Automatic duration is native code: it uses the greatest visual end and,
after media probing, an enabled audio end. Asset existence, image decoding,
audio probing, trim duration, and FFmpeg compatibility remain outside Python.

Relevant native diagnostics include `MVP-ASSET-*`, `MVP-AUDIO-ASSET`,
`MVP-AUDIO-ASSET-TYPE`, `MVP-AUDIO-SETTINGS`, `MVP-CLIP-*`,
`MVP-DURATION-EMPTY`, and `MVP-DURATION-TRUNCATED`.

## Authoring rules

Asset and clip IDs use the existing fixed-width allocator. Image and audio
assets share the `asset` namespace but retain separate generated prefixes.
Image and solid clips share the `clip` namespace. Registration preserves paths
exactly and performs no probing. Clips retain creation order. `to_dict()` makes
fresh JSON-compatible snapshots and omits absent optional values.

Image clips have image-only `sizing`, `crop`, `set_crop`, `clear_crop`, and
`transform` state. Solid-colour clips are separate types and cannot expose
those properties. Both types expose validated start, duration, layer,
visibility, and opacity. Python rejects non-finite numeric values, negative
starts, non-positive durations, boolean layers, invalid opacity, invalid anchor
space, and non-positive scale. Native validation still owns broader timeline
and media rules.

## Runtime verification

| Command | Result |
| --- | --- |
| `.venv/bin/python -m pytest python-tests/test_authoring_phase8b.py -q` | passed: 4 tests |
| `.venv/bin/python -m pytest python-tests -q` | passed: 124 tests, 4 adapter-gated skips |
| `.venv/bin/python -m mypy python/video_editor python-tests/test_typing.py python-tests/test_authoring_phase8b.py` | passed |
| `.venv/bin/python -m mypy.stubtest --ignore-unused-allowlist --allowlist python-tests/stubtest-private-hooks.txt video_editor` | passed |
| `cargo fmt --all -- --check` | passed |
| `cargo check --workspace` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace -q` | passed |
| `python3 crates/video-editor-cli/tests/schema_validation.py` | passed |
| `maturin develop` | passed |
| `maturin build` and wheel contents inspection | passed: authoring modules and `py.typed` present |
| clean wheel installed outside source tree | passed: authored image, solid, and audio project built, validated, prepared for a CPU frame, rendered CPU MP4, and FFprobe found audio |
| adapter-gated WGPU tests | skipped: no usable adapter |

The tested interpreter was CPython 3.13.5 on Linux x86_64. Rust was 1.96.1.
FFmpeg and FFprobe were available. The skipped WGPU tests do not weaken CPU
verification.

## Finalization correction — 2026-07-30

The ownership finalization changed the authoring contract after the initial
record above. Assets, clips, transforms, tracks, and audio tracks are factory
created only; direct construction raises a clear `TypeError`. Asset and node
representations are deterministic and omit owner identity. Track, transform,
and crop references are read-only, while validated `base_value` mutation is
preserved. Crop is one stable clip-owned node controlled by `has_crop`.

Asset and clip arguments are validated and normalized before IDs are reserved,
so failed creation leaves the builder dictionary and ID allocation state
unchanged. Audio uses one stable node: repeated `set_audio()` updates it,
`clear_audio()` disables serialization, and `has_audio` reports enablement.

This finalization pass passed `cargo fmt --all -- --check`, `cargo check
--workspace`, clippy with warnings denied, `cargo test --workspace`, schema
validation, `maturin develop`, the full Python suite (138 passed, 4
adapter-gated skips), mypy, and stubtest. `maturin build` passed, and a clean
temporary virtual environment installed the wheel and imported both
`video_editor` and `video_editor.authoring` outside the source tree. Strict
WGPU was run with `VIDEO_EDITOR_REQUIRE_WGPU=1` and failed because no compatible
adapter exists: this is an **expected environmental failure**, not a skip.

The committed CPU authoring tests assert exact interior pixels for solid colour,
image visibility, all five sizing modes, crop application and crop clearing,
position, scale, rotation, opacity, layer ordering, and ID tie-breaking. They
also verify automatic duration through native inspection (including hidden
clips, trimmed/timeline-offset audio, and mute), plus actual visual-only,
audio-enabled, and muted FFprobe stream behavior.

## Final verdict

Phase 8B complete.
