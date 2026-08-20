# Transitions

Transitions are ordered placements owned by one composition: `TransitionPlacement(id, outgoing, incoming, start, duration, definition)`. `outgoing` and `incoming` are sibling layer IDs; `start` and `duration` are local seconds. Validation checks endpoint ownership, duration, fit/overlap, IDs, tracks and effects. A nested composition owns its own placements and local timeline.

Every built-in below defaults `easing` to `Interpolation.EASE_IN_OUT`. It accepts the interpolation value/string used by the public animation API. `Animate` uses normalized 0..1 transition progress with strictly increasing keyframes that start at 0 and end at 1.

| Definition | Exact keyword parameters |
| --- | --- |
| `Crossfade` | `easing=Interpolation.EASE_IN_OUT` |
| `DirectionalPush` | `angle_degrees=0.0`, `distance=1.0`, `easing=Interpolation.EASE_IN_OUT`; angle finite degrees, distance non-negative. |
| `PushLeft` / `PushRight` / `PushUp` / `PushDown` | `distance=1.0`, `easing=Interpolation.EASE_IN_OUT`; fixed angles 180, 0, -90 and 90 degrees. |
| `ZoomCrossfade` | `outgoing_zoom=1.1`, `incoming_start_zoom=0.9`, `easing=Interpolation.EASE_IN_OUT`; both zooms positive. |
| `ZoomIn` | `amount=0.9`, `easing=Interpolation.EASE_IN_OUT`; `amount` is positive incoming-start zoom. |
| `ZoomOut` | `amount=1.1`, `easing=Interpolation.EASE_IN_OUT`; `amount` is positive outgoing zoom. |
| `BlurCrossfade` | `radius=12.0`, `easing=Interpolation.EASE_IN_OUT`; positive pixel radius. |
| `ZoomBlurTransition` | `radius=0.8`, `easing=Interpolation.EASE_IN_OUT`; non-negative zoom-blur radius. |
| `WhipPanLeft` / `WhipPanRight` | `distance=1.0`, `radius=12.0`, `easing=Interpolation.EASE_IN_OUT`; both non-negative, radius in pixels. |
| `CustomTransition` | `outgoing=None`, `incoming=None`, `default_easing=Interpolation.LINEAR`; at least one channel/effect is required. |

`CustomTransition` takes `TransitionLayer(opacity, position, scale, rotation, effects)` channels. Opacity is 0 through 1, position/scale serialize as points and rotation is finite degrees. Its effects are ordinary visual-effect canonical objects. CPU and WGPU consume the compiled presentation plan. Both paths support the catalog, but full visual parity beyond renderer tests is not verified.
