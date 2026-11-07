---
description: Transition placement ownership, endpoint restrictions, normalized channels, constructors and validation errors.
---

# Transitions

A transition is a placement owned by one composition. Its immutable definition
can be reused with other endpoints, starts and durations. See the
[transition guide](../guides/python/transitions-flashes-and-presets.md) for
rendered comparisons and a complete program.

## Placement

```text title="Python API signature"
composition.transitions.add(outgoing, incoming, definition, *, start, duration, id=None)
```

`outgoing` and `incoming` are different, live sibling `Layer` objects owned by
`composition`. The returned `TransitionPlacement` has `id`, `outgoing`,
`incoming`, `start`, `duration` and `definition`. The canonical form uses sibling
layer IDs for the two endpoints. `start` is finite, non-negative **composition-local
seconds**; `duration` is finite and positive. The whole transition interval must
fit inside both visible endpoint lifetimes. A layer cannot participate in
concurrent transitions in one composition; intervals may touch at a boundary.

### Endpoint sources and nested ownership

Image, video, shape, text and group clips are direct canonical endpoints.
High-level Python adapts `Color`/`SolidColor`, `Spectrum2D` and particle layers
when presentation or a transition requires it. A raw canonical `solid_color`,
`spectrum2d` or `particle_system` clip cannot be named as a direct endpoint.
An adapted source retains its own preparation requirements, including audio
analysis for Spectrum2D.

A composition placed in its parent is a group endpoint in that parent. A
transition between its children belongs to the child's transition collection
and local clock. Descendant IDs cannot be used to cross that ownership boundary.

## Definitions

Every built-in below defaults `easing` to `Interpolation.EASE_IN_OUT`.
Python requires an `Interpolation` enum or `CubicBezier` value; arbitrary
strings are not accepted by these constructors. Canonical JSON stores the
corresponding interpolation representation.

| Definition | Exact keyword parameters |
| --- | --- |
| `Crossfade` | `easing=Interpolation.EASE_IN_OUT` |
| `DirectionalPush` | `angle_degrees=0.0`, `distance=1.0`, `easing=Interpolation.EASE_IN_OUT`; angle finite degrees, distance non-negative and normalized to composition dimensions. |
| `PushLeft` / `PushRight` / `PushUp` / `PushDown` | `distance=1.0`, `easing=Interpolation.EASE_IN_OUT`; fixed angles 180, 0, -90 and 90 degrees. |
| `ZoomCrossfade` | `outgoing_zoom=1.1`, `incoming_start_zoom=0.9`, `easing=Interpolation.EASE_IN_OUT`; both zooms positive scale multipliers. |
| `ZoomIn` | `amount=0.9`, `easing=Interpolation.EASE_IN_OUT`; `amount` is positive incoming-start zoom. |
| `ZoomOut` | `amount=1.1`, `easing=Interpolation.EASE_IN_OUT`; `amount` is positive outgoing zoom. |
| `BlurCrossfade` | `radius=12.0`, `easing=Interpolation.EASE_IN_OUT`; positive Gaussian pixel radius. |
| `ZoomBlurTransition` | `radius=0.8`, `easing=Interpolation.EASE_IN_OUT`; non-negative zoom-blur radius, distinct from Gaussian pixel radius. |
| `WhipPanLeft` / `WhipPanRight` | `distance=1.0`, `radius=12.0`, `easing=Interpolation.EASE_IN_OUT`; both non-negative, radius in pixels. |
| `CustomTransition` | `outgoing=None`, `incoming=None`, `default_easing=Interpolation.LINEAR`; at least one channel/effect is required. |

## Normalized custom channels

`CustomTransition` takes `TransitionLayer(opacity, position, scale, rotation,
effects)` for each endpoint. Channels use `Animate(start_value, end_value,
easing=None)` or `Animate.keyframes(...)`. Keyframe progress is strictly
increasing, starts at 0 and ends at 1. Progress is
`(composition_local_time - placement.start) / placement.duration` within the
transition interval. Changing placement duration stretches the same definition.

| Channel | Canonical name | Meaning |
| --- | --- | --- |
| `opacity` | `opacity` | 0–1 multiplier on endpoint opacity; normal alpha compositing still applies. |
| `position` | `position_offset` | Normalized point offset added to the endpoint position. |
| `scale` | `scale_multiplier` | Positive scalar or positive point components multiplying endpoint scale. |
| `rotation` | `rotation_offset_degrees` | Finite degree offset added to endpoint rotation. |
| `effects` | `effects` | Ordinary visual-effect objects applied to the endpoint presentation. |

Position accepts `Point` or `(x, y)` tuples; scale additionally accepts a positive
scalar. Effect parameter keyframe times inside a transition definition use
normalized 0–1 progress rather than ordinary layer seconds. These presentation
channels overlay the endpoint's authored animation; they do not replace its
property tracks. An ordinary `Crossfade` is not a guaranteed 50/50 RGB midpoint:
both endpoint alphas and layer order determine the composite.

## Validation and support

Python collection checks reject wrong ownership, missing/identical endpoints,
unsupported sources, duplicate collection IDs and invalid start/duration values.
Semantic validation also checks interval fit, hidden endpoints, overlapping
participation, channel tracks, effect validity and combined effect-count limits.
Call `project.validate()` before media-dependent preparation when you need those
diagnostics separately.

| Diagnostic | Meaning |
| --- | --- |
| `VESTRA-TRANSITION-FIT` | The interval does not fit both endpoint lifetimes. |
| `VESTRA-TRANSITION-HIDDEN` | An endpoint clip is hidden. |
| `VESTRA-TRANSITION-CLIP` | A referenced canonical endpoint does not exist in this scope. |
| `VESTRA-TRANSITION-SOURCE` | The canonical source is not a direct endpoint. |
| `VESTRA-TRANSITION-CONFLICT` | One layer participates in overlapping placements. |
| `VESTRA-LIMIT-TRANSITION-EFFECTS` | Authored and transition effects together exceed the per-clip limit. |

CPU and WGPU consume the compiled presentation plan. Both paths dispatch the
catalog, but full WGPU visual parity is not verified. The guide's comparisons
are CPU evidence; see the [feature matrix](feature-support.md).
