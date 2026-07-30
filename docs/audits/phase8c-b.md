# Phase 8C-B audit

## Public authoring model

Phase 8C-B adds stable ordered `clip.effects` and `builder.post_effects`
collections. Both expose a tuple `items`, retain identity, and serialize in
declaration order. Effects are factory-created handles with read-only `id` and
`kind`; equality includes builder owner, collection scope, ID, and kind.
Generated IDs are fixed-width `effect-000001` values. Each clip owns an ID
scope; global post-effects use a separate scope. Collections are created only
by their owning clip or builder, so their public constructors expose no owner,
ID allocator, or scope token. Creation validates values
before reserving an ID, so failures do not alter output or consume identifiers.

The supported factories are `add_brightness`, `add_contrast`,
`add_saturation`, `add_tint`, `add_gaussian_blur`, `add_directional_blur`,
`add_zoom_blur`, `add_glow`, `add_chromatic_aberration`, `add_vignette`,
`add_sharpen`, and `add_color_adjust` on both collections, plus
`add_camera_shake` and `add_motion_blur` on clip collections only. The latter
two are transform-aware clip effects and intentionally cannot be global.

| Effect | Canonical parameters |
| --- | --- |
| brightness, contrast, saturation | `amount` track |
| tint | `colour`, `amount` track |
| gaussian blur | `radius` track |
| directional blur | `radius`, `angle_degrees` tracks |
| zoom blur | `radius` track, `samples`, `anchor`, `direction` |
| glow | `threshold`, `radius`, `intensity` tracks, `colour` |
| chromatic aberration | `amount`, `angle_degrees` tracks |
| vignette | `amount`, `radius`, `softness` tracks, `colour` |
| sharpen | `amount`, `radius` tracks |
| color adjust | `exposure`, `gamma`, `black_point`, `white_point` tracks |
| camera shake | `ActiveInterval`, four tracks, `seed`, `attack`, `decay` |
| motion blur | `intensity`, `shutter_angle`, `max_radius` tracks, `samples` |

`ActiveInterval(start=0.0, duration=None)` serializes to native `start` and
optional `duration`. It is immutable, finite, clip-local, and half-open.
Numeric effect parameters reuse Phase 8C-A `ScalarTrack`, named interpolation,
and cubic Bézier keyframes; there is no second animation model.

`SharpenEffect`, `VignetteEffect`, and `ChromaticAberrationEffect` are now
independent public classes. Their tracks map directly to their canonical fields,
so no unrelated inherited property leaks into their API. Python validates the
Rust representation ranges for zoom-blur and motion-blur `u8` samples and the
camera-shake `u64` seed before reserving an ID. Rejected values leave the
collection and its generated-ID counter unchanged.
The schema now mirrors that `u64` seed maximum, so direct JSON validation and
Python authoring reject the same unrepresentable values before Rust parsing.

Ordinary clip-effect tracks use clip-relative seconds. Camera-shake tracks use
seconds relative to the active interval start. Global post-effect tracks use
project-relative seconds. The CPU motion-blur test compares two otherwise
identical moving clips, with the effect as the only difference. The committed
CPU video test uses two clips, screen blending, ordered clip effects, an
animated brightness track, and ordered global post-effects; it validates,
prepares, renders, probes a video-only MP4, and checks cleanup.

The public API test table records the concrete type and complete property set
for all fourteen effects. It catches inherited properties as well as missing
ones, mutates every scalar track, checks its canonical field, and rejects track
replacement. The negative typing fixture rejects the removed inherited
properties and invalid integer types. Runtime tests separately prove a normal clip effect at a clip-local
timestamp, a global post-effect at a project-relative timestamp, and a
camera-shake parameter that starts at its active interval.

Image and solid-colour clips expose `BlendMode`: `normal`, `add`, `screen`,
`multiply`, and `overlay`. Normal is omitted to preserve existing static output;
non-normal values serialize as `blend_mode`.

## Validation boundary and verification

Python checks public types, booleans-as-numbers, finiteness, colours, immutable
collection identity, interval shape, and post-effect category. Native validation
remains authoritative for effect ranges, interval fitting, keyframe timing,
interactions, and renderer preparation.

| Command | Result |
| --- | --- |
| `python-tests/test_authoring_effects.py` | passed: 31 tests |
| `mypy python/video_editor python-tests/test_typing.py` | passed |
| `stubtest` | passed |
| full Python suite | passed: 185 tests, 4 adapter-gated skips |
| schema validation | passed |
| formatting | passed |
| Cargo check, clippy, workspace tests | passed |
| all variants native round trip | passed |
| individual CPU effect frames | passed: stable changed-channel assertions for brightness, contrast, saturation, tint, Gaussian/directional/zoom blur, glow, chromatic aberration, sharpen, color adjust, and camera shake; dedicated vignette and motion-blur regions |
| CPU frame smoke with animated brightness and ordered post-effects | passed |
| effect and post-effect declaration order | passed: distinct exact CPU pixels |
| deterministic CPU pixels for all five blend modes | passed |
| CPU effect video through FFmpeg | passed: ten frames, one second, video-only |
| wheel build and clean-wheel effect smoke | passed |
| normal WGPU tests | skipped: no compatible adapter |
| strict WGPU tests | expected environmental failure: no compatible adapter |

## Environment and final verdict

Verification used CPython 3.13.5, Rust and Cargo 1.96.1, Maturin 1.14.1, and
FFmpeg/FFprobe 7.1.5 on Linux x86_64. The normal WGPU tests skip cleanly when
adapter discovery finds no compatible device. The strict command was also run;
it fails with `WGPU adapter request returned no compatible adapter`, which is
an environmental failure, not a parity result. WGPU compilation and
capability-oriented workspace coverage pass, but this audit makes no claim of
adapter-backed CPU/WGPU runtime parity in this environment.

The wheel smoke installed the built wheel in a separate environment outside the
source tree, imported all public effect types, authored clip and post effects,
resolved runtime hints, rendered a CPU frame and a ten-frame CPU video, and
confirmed `py.typed` remains in the wheel. The requested cache scan produced no
`__pycache__`, `.pyc`, or `.pyo` files.

No transitions, presets, flashes, or other later-phase authoring APIs were
introduced.

## Verdict

Phase 8C-A complete

Phase 8C-B complete
