# Phase 9D: public audio API finalization

Phase 9D closes the public conformance work for the schema-v2 audio timeline.
It does not change mixer semantics.

## Final contract

The Rust SDK keeps the application DTO boundary. It exports inspection DTOs
`InspectAudio`, `InspectAudioTrack`, `InspectAudioClip`, and
`InspectAudioGainKeyframe`, plus the `AudioGainInterpolation` and
`AudioFadeCurve` enums. Inspection preserves declared track, clip, and
keyframe order. It reports authored audio even when `output.audio` is false.
It exposes track ID, mute, gain, clip IDs, asset IDs, project start/end,
effective trims, clip mute/gain, fades and curves, and automation keyframes.
It does not expose FFmpeg graph implementation details.

`video_editor.authoring` exports `AudioTimeline`, `AudioTrack`, `AudioClip`,
`AudioGainKeyframe`, `AudioGainInterpolation`, and `AudioFadeCurve`.
`builder.audio` is a stable builder-owned object. It serializes only when it
has tracks, is independent of `output_audio`, and returns immutable tuple
snapshots for tracks, clips, and automation keyframes. Existing handles retain
their identity across snapshots. `AudioTrack` and `AudioClip` equality and
hashing use builder ownership, type, and canonical ID, so equal IDs from two
builders are distinct handles.

Audio tracks expose `id`, mutable `gain` and `mute`, `clips`, and `add_clip`.
Audio clips expose `id`, `asset`, mutable `start`, `trim_start`, `trim_end`,
`gain`, `mute`, `fade_in`, `fade_out`, `fade_in_curve`, `fade_out_curve`, and
immutable `gain_automation`. Every numeric mutation rejects bool, non-finite,
and negative values. Trim endpoints remain locally coherent. Native validation
still owns cross-field project and media constraints. Whole-envelope
replacement uses `set_gain_automation`; `clear_gain_automation` removes the
optional canonical field.

`AudioTimeline.crossfade(outgoing, incoming, *, curve=AudioFadeCurve.EQUAL_POWER)`
requires an existing positive overlap and an authoring-resolvable outgoing end.
It uses the full overlap, writes normal fade fields, never creates a crossfade
node, and does not move clips or alter trims. It verifies ownership and fade
conflicts before committing either change.

Native dictionary and JSON round trips stabilize for an overlapping,
automated, equal-power project. Canonical automation is
`{"keyframes": [{"time": ..., "gain": ..., "interpolation": "linear" | "hold"}]}`.
Equal-power fades serialize as ordinary `fade_in_curve` or `fade_out_curve`
values of `equal_power`; linear is the omitted default.

## Semantics and limits

`output.audio` controls stream muxing only. It does not erase inspection or
change structural duration. Muted and zero-gain clips also remain structural.
Semantic validation owns IDs, gains, fade values, automation ordering, and
authored limits. Preflight owns source-duration checks and sample-resolution
checks. The execution stage owns the deduplicated unique-source budget.

The documented limits are 256 authored tracks, 4,096 authored clips, 16,384
authored gain keyframes, and 128 unique audible resolved sources per render.
Authored seconds are floating point. The 48,000 Hz mixer rounds nonnegative
times to the nearest sample, ties upward, so the maximum quantization error is
half a sample, about 10.42 microseconds. Adjacent automation keys must resolve
to different samples. Equal-power fade-in is `sin(pi*u/2)` and fade-out is
`cos(pi*u/2)`; matching envelopes satisfy `g_in^2 + g_out^2 = 1`.

Audio gain automation uses source-keyframe interpolation. The interpolation on
keyframe `i` controls the segment from keyframe `i` to keyframe `i + 1`.
This deliberately differs from visual animation tracks, where a keyframe's
interpolation controls the segment ending at that keyframe. The final audio
gain keyframe has no following segment, so its interpolation has no effect and
its gain holds through the selected clip end. The project-format specification
now includes a compact canonical JSON example with a linear `0.0..0.5`
segment, a hold `0.5..1.0` segment, and a final value held to clip end.

## Evidence

New executable schema examples are `audio-static-mix.json`,
`audio-gain-automation.json`, and `audio-equal-power-crossfade.json`. The
public Python music-mix example is `examples/python/07_music_mix.py`. It uses
two tracks, overlapping clips, a reused source, static gain, automation, an
equal-power crossfade, inspection, preparation, and CPU rendering.

The definitive example keeps its explicit three-second project duration. Its
outgoing clip spans `[0.0, 2.0]`; its incoming clip now spans `[1.5, 3.0]`
with `trim_end=1.5`, leaving a visible 0.5-second overlap without extending
beyond the project. The incoming automation is `(0.0, 0.0, linear)`, `(0.25,
1.0, hold)`, and `(0.5, 0.8, linear)`. Therefore the `hold` on the second
keyframe controls the real `0.25..0.5` segment. The final `linear` is unused,
and the gain `0.8` holds to the incoming clip end. The example asserts both a
valid report and zero warnings. Its final validation passed with zero warnings
and no codes, inspection reported audio with two tracks and three clips, and
the CPU prepared render returned `audio_present=True`.

Python's top-level package now exports the full inspection family:
`InspectAudio`, `InspectAudioTrack`, `InspectAudioClip`, and
`InspectAudioGainKeyframe`. The last DTO was already in the native extension
and `_native.pyi`; this closure adds it to the top-level import and `__all__`.
The public API-contract test asserts all four names. The Phase 9C inspection
test also confirms that a gain-automation item is an instance of the public
`InspectAudioGainKeyframe` class rather than relying on a private module path.

`scripts/verify-phase9-wheel.py` is the checked-in isolated-wheel smoke. It
requires an installed wheel, checks that imports resolve from `site-packages`
and that `py.typed` exists, then authors a public two-tone project with the
same automation and equal-power crossfade shape. It validates, inspects,
prepares on CPU, renders MP4/AAC, checks FFprobe's AAC/48 kHz/stereo stream,
decodes f32le PCM with FFmpeg, and uses standard-library tone correlation.
The smoke requires early 440 Hz correlation to exceed 880 Hz correlation and
late 880 Hz correlation to exceed 440 Hz correlation.

The public Rust SDK test now loads, validates, inspects, prepares, and renders
a multi-track project with automation and equal-power fades. Python tests cover
clip mutation validation, stable identity, immutable snapshots, handle
ownership/equality, automation clearing, declared-order inspection, native
dict round trips, and JSON round trips.

On this Linux CPython 3.13.5 environment, the following passed:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- `cargo test -p video-editor --test public_sdk --test public_exports`
- `python3 crates/video-editor-cli/tests/schema_validation.py`
- `maturin develop`
- `.venv/bin/python -m pytest python-tests` with 258 passed and 4 WGPU skips
- `.venv/bin/python -m mypy python/video_editor python-tests/test_typing.py`
- `.venv/bin/python -m mypy.stubtest --ignore-unused-allowlist --allowlist python-tests/stubtest-private-hooks.txt video_editor`
- `maturin build`

An isolated installed CPython 3.13 wheel imported from `site-packages`, found
`py.typed`, built an authored multi-track Phase 9 mix, validated, inspected,
prepared CPU rendering, rendered MP4 with AAC, and verified an AAC, 48 kHz,
stereo stream with FFprobe. FFmpeg decoded nonempty f32le PCM. The lightweight
signal check measured early `440=0.707 > 880=0.000` and late
`880=0.707 > 440=0.000`, which proves the public packaged workflow carries the
authored crossfade direction into the decoded output. FFmpeg and FFprobe were
7.1.5. WGPU tests were adapter-gated and skipped where no compatible adapter
was available.

Phase 9D is complete.

Phase 9 is complete.

The final schema-v2 audio model, static mixer, gain automation, equal-power
fades/crossfades, inspection API, Rust SDK integration, Python authoring API,
typing, examples, packaging, and documentation are internally consistent and
ready to serve as the stable audio foundation for downstream work.
