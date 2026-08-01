# Phase 8C-D audit

## Architecture

Presets are canonical schema-version 1 `clip.preset` values. Python writes the
five native discriminators, then Rust compilation creates transform
contributions and generated effects. Python does not expand presets, calculate
frames, or add hidden state.

| Preset | Python factory | Clip type | Fields | Default duration | Native output |
| --- | --- | --- | --- | --- | --- |
| `slow_drift` | `clip.presets.apply_slow_drift` | image | `intensity`, `start`, `duration` | remaining clip | position and scale contribution |
| `zoom_punch` | `clip.presets.apply_zoom_punch` | image | `intensity`, `start`, `duration` | 0.35 s | scale contribution |
| `impact` | `clip.presets.apply_impact` | image | `seed`, `intensity`, `start`, `duration` | 0.28 s | scale, shake, chromatic aberration, tint |
| `heavy_impact` | `clip.presets.apply_heavy_impact` | image | `seed`, `intensity`, `start`, `duration` | 0.4 s | scale, shake, directional blur, chromatic aberration, tint |
| `focus_reveal` | `clip.presets.apply_focus_reveal` | image | `intensity`, `start`, `duration` | 0.8 s | scale, blur, sharpen |

`intensity` defaults to 1.0 and is in `0..=2`; `start` defaults to 0.0;
`duration` is omitted by default. Impact variants require an unsigned 64-bit
`seed`. Clip-local preset timing moves with the clip. Generated effects run
before manually authored effects. Presets combine with authored transform
tracks, blend modes, transitions, flashes, and post-effects through the native
plan. A clip accepts one preset; a second application fails until `clear()` is
called, so no authored state is overwritten silently.

## Timeline helpers

`builder.timeline` is a stable, non-serializing builder-owned object.
`shift_clip` and `shift_clips` change only `clip.start`; keyframes remain
clip-local and transitions, flashes, and global post-effect keyframes stay at
their project timestamps. Batch shifts validate all ownership and resulting
starts before mutation, deduplicate handles in input order, and preserve
relative spacing. `add_crossfade_between` uses the beginning of an existing
overlap and rejects an overlong duration. It never moves or trims clips.

## Regression cleanup

The animation lifecycle comment now names the actual samples: base value,
before first keyframe, first keyframe, midpoint, and final keyframe. CPU tests
cover a red standalone flash over grey through linear fade-in, plateau, and
fade-out. They also assert channel behaviour for a non-white flash and
draw-key-ordered overlapping flashes. Existing animated brightness coverage
remains deterministic colour-path coverage.

## Focused verification

`test_authoring_preset_frames.py` adds five CPU frame tests using the
asymmetric image fixture. Slow drift proves the clip-local start and its
position/scale contribution; zoom punch proves peak and return; seeded impact
proves the generated chromatic/tint pulse; heavy impact proves a distinct
stronger native frame; focus reveal proves blur-to-focused progression.

The same file proves all three finite invalid interval cases preserve authored
data and report native `MVP-EFFECT-INTERVAL` at either
`/visual/clips/0/preset/start` or `/visual/clips/0/preset/duration`. Its full
project test includes solid and image clips, transform/opacity/crop animation,
an animated clip effect, screen blending, a preset, an explicit clip shift,
crossfade, flash, post-effect, global audio, CPU prepared frames, FFmpeg, and
FFprobe. The probe finds a 32×24, 50-frame video stream and an audio stream;
the temporary output is absent.

`test_authoring_presets_timeline.py` resolves runtime hints and inspects every
preset factory plus the timeline helpers, and confirms their constructors stay
builder-owned. `test_typing.py` covers impact and both shift helpers; its
negative fixture rejects a solid clip in the image-only helper, solid preset
access, and replacement of helper collections. README now shifts before
creating the helper transition and explicitly documents that existing
transitions, flashes, and post-effect keyframes do not move.

Focused Stage 1 tests: 28 passed. Full Python: 219 passed, 4 adapter-gated WGPU
skips. `cargo fmt --all -- --check`, `cargo check --workspace`, strict Clippy,
workspace tests, schema validation, mypy, stubtest, `maturin develop`, and
`maturin build` passed. Python is 3.13.5, Rust/Cargo 1.96.1, Maturin 1.14.1,
and FFmpeg/FFprobe 7.1.5. Normal WGPU remains adapter-gated; strict WGPU was
not rerun in this environment. The wheel contains `py.typed` and the authoring
modules; the release-candidate clean-wheel exercise belongs to Phase 8C-E.

## Verdict

Phase 8C-D complete.
