# Phase 8C-C audit

## Canonical model

Python authoring now covers the schema-version 1 coordinated transition set.

| Canonical type | Public class | Factory | Static parameters | CPU | WGPU |
| --- | --- | --- | --- | --- | --- |
| `crossfade` | `CrossfadeTransition` | `add_crossfade` | none | yes | yes |
| `zoom_crossfade` | `ZoomCrossfadeTransition` | `add_zoom_crossfade` | `outgoing_zoom`, `incoming_start_zoom` | yes | yes |
| `flash_cut` | `FlashCutTransition` | `add_flash_cut` | `colour`, `intensity` | yes | yes |
| `directional_push` | `DirectionalPushTransition` | `add_directional_push` | `angle_degrees`, `distance`, `blur_radius` | yes | yes |
| `zoom_blur` | `ZoomBlurTransition` | `add_zoom_blur` | `outgoing_zoom`, `incoming_start_zoom`, `blur_radius` | yes | yes |

Transitions use project-relative `start + duration`, canonical interpolation,
and static variant parameters. `outgoing` and `incoming` are immutable image
clip references. `builder.transitions` keeps insertion order and exposes a
tuple of handles.

The builder rejects non-image handles, foreign-builder handles, same-clip
relationships, non-finite timing, and invalid Python-representable parameter
values. Native validation owns clip visibility, interval fit, and overlapping
transition associations. Later clip mutation never changes a transition.
This split is deliberate: `visible`, `start`, and `duration` remain mutable on
clips, so the collection cannot make a permanent timing promise at creation.

Flashes live in `builder.flashes`. A flash has canonical `id`, `start`,
`duration`, `colour`, `opacity`, `fade_in`, `fade_out`, and `layer` fields.
Times are project-relative. Overlapping flashes remain valid and draw in
native draw-key order.

Transition and flash IDs use separate builder-local namespaces. Failed
construction happens before reservation, so generated IDs remain contiguous.

## Renderer order

The compiler emits visible clips, applies transition contributions, appends
flash layers, then runs global post-effects after composition.

## Verification

Regression cleanup now uses the asymmetric `wgpu-small-rgba.png` fixture with
exact interior regions. Anchor, crop, and position tests check fixed pixels at
known times. Gaussian blur checks edge-spread values at start, midpoint, and
end. Brightness checks the complete channel progression.

`python-tests/test_authoring_transitions_flashes.py` has 7 focused tests and covers all five canonical
transition discriminators, concrete return classes, collection identity,
serialization, transactional IDs, ownership, same-clip rejection, later
hidden-clip validation, interval fitting, conflicts, snapshots, and flash
ordering. `python-tests/test_authoring_transition_flash_frames.py` has 5 focused frame tests and checks
crossfade and flash pixels, all variant midpoint behaviour, and a CPU video
with a transition, flash, clip effect, post-effect, and screen blend mode.

The CPU video probe reports one `video` stream at 8x6 with 20 frames and no
temporary output. Phase 8C-D re-ran the full Python suite: 209 passed and 4
adapter-gated WGPU tests skipped. `cargo fmt --all -- --check`, `cargo check
--workspace`, strict workspace Clippy, schema validation, mypy, stubtest, and
wheel build passed. The wheel contains `py.typed`. Python was 3.13.5, Rust was
1.96.1, Maturin was 1.14.1, and FFmpeg/FFprobe were 7.1.5. Normal WGPU status
is adapter-gated; strict WGPU was not rerun in this phase, so no new strict
runtime claim is made.

## Verdict

Phase 8C-C complete
