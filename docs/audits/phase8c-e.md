# Phase 8C-E audit

## Scope

Phase 8C-E stabilizes the existing Phase 8 API; it adds no feature family.

## Public API and lifecycle

`video_editor.authoring.__all__` is explicit and tested. It exports builder,
value objects, assets, clips, tracks/keyframes, effects, transitions, flashes,
presets, and timeline helpers; it excludes owner, allocator, scope, serializer,
and transaction implementation details. Value objects are directly
constructible and immutable. Assets, clips, tracks, effects, collections,
transitions, flashes, audio, presets helpers, and timeline helpers are
builder-owned and reject direct construction.

| Object | Mutable? | Stable identity? | Builder-owned? | Serialized directly? |
| --- | --- | --- | --- | --- |
| ProjectBuilder | yes | yes | n/a | snapshot only |
| ImageAsset / ImageClip | clip yes; asset no | yes | yes | yes |
| Transform / ScalarTrack | yes | yes | yes | through clip |
| Keyframe / Preset | no | value | no | yes |
| Effect / Transition / Flash | yes | yes | yes | yes |
| Timeline / AudioTrack | yes | yes | yes | helper / yes |

The graph is mutable authoring state → isolated dictionary snapshot → immutable
native `Project`. Native defaults mean the minimal authoring dictionary is not
textually identical to its first native serialization.

## Conformance evidence

`test_authoring_phase8e.py` verifies the explicit export surface, constructor
policy, runtime hints/signatures, static/animation/effect/transition-flash/
preset/audio/full native round trips, snapshots, failed-operation atomicity,
cross-scope IDs, deterministic complete builders, and no hidden repair after
dependent mutation. `phase8-authoring-conformance.md` records every relevant
schema-v1 feature and intentional deferrals. The Stage 1 full project provides
prepared-frame and CPU-video coverage including audio and safe cleanup.

## IDs, transactions, and error boundaries

The test matrix verifies deterministic fixed-width allocation, independent
builders, same-scope duplicate rejection, valid cross-scope effect ID reuse,
and that invalid asset, clip-shift, transition-helper, audio, effect,
integer-effect parameter, flash, preset, and transition operations leave the
dictionary and allocation sequence unchanged. Collection declaration order is
preserved; helpers deduplicate only their explicit clip input and do not reorder
canonical collections.

Python raises type/value/ownership errors for malformed typed calls. Native
parsing accepts schema-v1 dictionaries. Native validation intentionally reports
semantic errors such as keyframe range/order, transition fit or hidden
participants, and preset `MVP-EFFECT-INTERVAL` timing. Preflight reports media
and backend availability; rendering reports FFmpeg and safe-publication errors.

## Integration and examples

The committed full-project prepared-frame/video test samples preset activity,
the transition/flash interval, and authored animated effects, then verifies
32×24 CPU output with 50 frames, both media streams, and no temporary residue.
`examples/python/01_static_project.py` through `06_full_project.py` use only
public APIs and are syntax/import-smoked. They progress from static projects
through animation, effects, transitions/flashes, presets/timeline helpers, and
the full audio/render pipeline.

## Environment and verdict

CPython 3.13.5, Rust/Cargo 1.96.1, Maturin 1.14.1, and FFmpeg/FFprobe 7.1.5
were exercised on Linux x86_64. The full Python suite passed (230 passed, four
adapter-gated skips); mypy and stubtest passed. Formatting, workspace check,
strict Clippy, workspace tests, and schema validation passed. `maturin build`
produced a wheel containing every authoring module, `py.typed`, and the native
extension. A new isolated virtual environment installed that wheel with no
source-tree import path, resolved public hints, authored a complete project,
validated and built it, prepared a CPU frame, rendered CPU video, confirmed
FFprobe video and audio streams, and verified temporary cleanup.

Normal WGPU tests skipped because no compatible adapter was available. The
strict selection failed all four adapter-backed tests with `WGPU adapter request
returned no compatible adapter`; this is an expected environmental failure, not
a skip or a runtime-parity claim. WGPU compilation and normal capability tests
remain covered by the workspace and adapter-gated suites.

## Final verdict

Phase 8C-E complete
