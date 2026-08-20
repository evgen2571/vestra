# Transitions

Transitions are ordered placements owned by one composition. A placement has `id`, `outgoing`, `incoming`, `start`, `duration`, and `definition`. Times are local seconds. Both endpoints must belong to the composition and satisfy fit/overlap validation. Nested compositions may own transitions.

Built-ins produce generic outgoing/incoming presentation channels. `easing` defaults to `Interpolation.EASE_IN_OUT`. `Animate` uses normalized progress from 0 to 1, with strictly increasing keyframes that start at 0 and end at 1.

| Python definition | Parameters and defaults |
| --- | --- |
| `Crossfade` | `easing=EASE_IN_OUT` |
| `DirectionalPush` | `angle_degrees=0`, `distance=1`, `easing=...`; angle finite, distance non-negative. |
| `PushLeft`, `PushRight`, `PushUp`, `PushDown` | `distance=1`, `easing=...`; fixed angles 180, 0, -90, 90 degrees. |
| `ZoomCrossfade` | `outgoing_zoom=1.1`, `incoming_start_zoom=0.9`, `easing=...`; zooms positive. |
| `ZoomIn`, `ZoomOut` | `amount=0.9` / `1.1`, `easing=...`. |
| `BlurCrossfade` | `radius=12`, `easing=...`; radius positive. |
| `ZoomBlurTransition` | `radius=0.8`, `easing=...`; radius non-negative. |
| `WhipPanLeft`, `WhipPanRight` | `distance=1`, `radius=12`, `easing=...`; both non-negative. |
| `CustomTransition` | `outgoing=None`, `incoming=None`, `default_easing=LINEAR`; at least one channel/effect required. |

`CustomTransition` accepts `TransitionLayer(opacity, position, scale, rotation, effects)`. Opacity is 0 through 1; position and scale serialize as points; rotation is finite degrees. Transition effects use normal visual-effect canonical objects. Validation checks ids, endpoints, duration, timing, tracks, and effects.

CPU and WGPU consume the compiled presentation plan. The catalog and placement behavior work on both paths; exact visual parity beyond covered renderer tests is not fully verified.
