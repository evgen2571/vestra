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

`python-tests/test_authoring_transitions_flashes.py` covers all five canonical
transition discriminators, concrete return classes, collection identity,
serialization, transactional IDs, ownership, same-clip rejection, later
hidden-clip validation, interval fitting, conflicts, snapshots, and flash
ordering. `python-tests/test_authoring_transition_flash_frames.py` checks
crossfade and flash pixels, all variant midpoint behaviour, and a CPU video
with a transition, flash, clip effect, post-effect, and screen blend mode.

The CPU video probe reports one `video` stream at 8x6 with 20 frames and no
temporary output. The full Python suite passes with adapter-gated WGPU tests
skipped when no adapter is available. Formatting, workspace checking, Clippy,
schema validation, mypy, wheel build, and a clean-wheel authoring smoke pass.
The strict WGPU command was also run and failed with `WGPU adapter request
returned no compatible adapter`; this is an environmental result, not a runtime
parity claim.

## Verdict

Phase 8C-C complete
