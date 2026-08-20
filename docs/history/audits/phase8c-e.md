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
dependent mutation. `test_authoring_presets_timeline.py` also protects the
README workflow: shifting an incoming clip from 0.50 to 0.75 seconds leaves a
0.25-second overlap with an outgoing 0.00-to-1.00-second clip, then the helper
adds a valid crossfade. `phase8-authoring-conformance.md` records every current
schema-v1 field family. No current schema-v1 field is raw-only. The Stage 1 full
project provides prepared-frame and CPU-video coverage including audio and safe
cleanup.

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

The final-state checks ran on Linux x86_64 with CPython 3.13.5, Rust/Cargo
1.96.1, Maturin 1.14.1, and FFmpeg/FFprobe 7.1.5.

| Gate | Final-state result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo check --workspace` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` and the requested `video-editor` and `video-editor-python` package/public-SDK/public-export selections | passed |
| `python3 crates/video-editor-cli/tests/schema_validation.py` | passed |
| `maturin develop` | passed with the repository CPython 3.13 environment |
| Full Python suite | 231 passed, 4 adapter-gated skips |
| Mypy | passed for 18 source files and 17 modules |
| Stubtest | passed |
| CPU complete-project render and FFprobe video/audio-stream checks | passed through the full Python suite |
| Normal WGPU selection | 4 adapter-gated skips in the full Python suite, because no compatible adapter was available |
| Strict WGPU selection | 4 failed with `WGPU adapter request returned no compatible adapter`; expected environmental failure, not a skip |
| `maturin build` | passed after disk space was restored |
| Clean-wheel smoke | passed from an isolated virtual environment outside the source tree: wheel import, public authoring imports and hints, `py.typed`, complete project construction, validation, CPU frame preparation, CPU video render, FFprobe video/audio streams, and temporary-output cleanup |

The source tree contains no Python bytecode caches, `.pytest_cache`, or
`.mypy_cache` after cleanup. The existing `target` directory is a normal build
tree and was retained.

## Completion definition

Phase 8 complete means complete typed Python authoring coverage for the current
schema-version 1 project model. `Project.from_dict()` is a lower-level
construction path for the same model, not access to nonexistent features.
Video assets, multi-track audio and mixer features, nested compositions, and
audio-reactive visual systems require future native project-model and schema
work.

## Final verdict

Phase 8C-E complete.
