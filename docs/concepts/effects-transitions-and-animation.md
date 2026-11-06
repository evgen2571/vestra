# Effects, transitions, animation, flashes, and presets

These features act at different levels of the project. Keeping the levels
separate makes complex edits easier to reason about.

| Feature | What it changes |
| --- | --- |
| Animation | A property value over time |
| Effect | The appearance or processing of one layer or the project output |
| Transition | How two sibling layers exchange presentation over an interval |
| Flash | A short root-owned colour overlay |
| Preset | Reusable cinematic intent applied to an eligible layer |

Animation is the time-varying input. For example, keyframes can change opacity,
position, scale, rotation, or an effect parameter. An effect consumes those
values while processing a layer or output. A transition owns presentation
channels for its outgoing and incoming endpoints, so its animation is relative
to transition progress rather than a general layer property track.

Transitions belong to a composition and connect two layers in that same
composition. Composition layers can be endpoints, and a child composition can
have its own transitions between its children. A transition does not move
layers into a new composition.

Flashes are root overlays with their own start, duration, opacity, and fades.
They are useful for a beat or burst that should sit over the rendered project.
Presets are reusable values for supported image layers. A layer can have one
cinematic preset at a time. Presets do not replace ordinary transforms or
effects.

Before the first keyframe, a track evaluates to its base value. Between
keyframes it interpolates, and after the final keyframe it holds that final
value. The high-level API lowers all of these descriptors into the canonical
project before rendering. Their authored ownership stays distinct even when
the final frame combines them.

## Interpolation

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
