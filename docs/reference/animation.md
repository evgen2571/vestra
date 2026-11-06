---
description: Exact keyframe clocks, interpolation enums and canonical JSON timing curves.
---

# Animation tracks and interpolation

Interpolation controls the segment **ending at** a keyframe. Put the easing
on the destination keyframe, not the starting one. The first keyframe takes
effect at its time; there is no interpolation from the base value to it.
Times must be non-negative and strictly increasing. Layer tracks use
layer-local seconds; timed effects use effect-local seconds. Transition
channels instead use normalized progress from 0 to 1.

| Python `Interpolation` member | JSON value | Behavior between keyframes |
| --- | --- | --- |
| `LINEAR` | `"linear"` | Constant rate; the Python keyframe default. |
| `HOLD` | `"hold"` | Keep the preceding value, then jump at the destination time. |
| `EASE_IN` | `"ease_in"` | Start slowly and accelerate (`t²`). |
| `EASE_OUT` | `"ease_out"` | Start quickly and decelerate (`1 − (1 − t)²`). |
| `EASE_IN_OUT` | `"ease_in_out"` | Smooth start and finish (`t²(3 − 2t)`). |

Here `t` is progress through the segment, from 0 to 1. Scalar values and each
component of point/crop values use the same curve. For example, fade a rectangle
in over half a second, then hold it visible:

```python
from vestra import Interpolation, Project, Rectangle

project = Project(
    size=(320, 180),
    fps=30,
    duration=2,
)
layer = project.root.add(Rectangle(width=160, height=90, fill="#ffffff"))
layer.opacity = 0
layer.opacity.keyframe(0, 0)
layer.opacity.keyframe(0.5, 1, interpolation=Interpolation.EASE_OUT)
```

The equivalent JSON clip `opacity` field is:

```json
{
  "base_value": 0,
  "keyframes": [
    {"time": 0, "value": 0, "interpolation": "linear"},
    {"time": 0.5, "value": 1, "interpolation": "ease_out"}
  ]
}
```

For a custom timing curve, pass `CubicBezier(x1, y1, x2, y2)` instead of an
enum member:

```python
from vestra import CubicBezier

layer.transform.position.keyframe(0, (0.25, 0.5))
layer.transform.position.keyframe(
    1, (0.75, 0.5), interpolation=CubicBezier(0.42, 0, 0.58, 1)
)
```

Its destination JSON keyframe is:

```json
{
  "time": 1,
  "value": {"x": 0.75, "y": 0.5},
  "interpolation": {"type": "cubic_bezier", "x1": 0.42, "y1": 0, "x2": 0.58, "y2": 1}
}
```

All four controls must be finite; `x1` and `x2` must be in `[0, 1]`.
Y controls outside that interval can overshoot, so keep the animated result
within the property's supported range. In Python, property keyframes accept
`Interpolation` or `CubicBezier` values, rather than strings.


For a progressive explanation and rendered result, use [animation and keyframes](../learn/animation.md).
See [timeline and time](../concepts/timeline-and-time.md) for source and nested clocks.
