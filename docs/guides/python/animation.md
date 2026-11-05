# Animate properties

High-level properties hold a base value and optional keyframes. Keyframe times
are seconds local to the layer or effect that owns the property.

```python
from vestra.sources import Image

layer = project.root.add(Image("assets/photo.png"), duration=5)
# Positions are normalized canvas coordinates; times are layer-local seconds.
layer.transform.position.keyframe(0, (0.35, 0.5))
layer.transform.position.keyframe(2, (0.65, 0.5))
layer.transform.position.keyframe(4, (0.5, 0.5))
layer.opacity.keyframe(0, 0.0)
layer.opacity.keyframe(0.5, 1.0)
```

Before the first keyframe, the base value applies. Between keyframes, Vestra
interpolates using the segment's ending keyframe interpolation. At and after
the final keyframe, Vestra holds the final keyframe value. Each keyframe can
use an `Interpolation` value or a `CubicBezier` easing curve. Keep keyframe
times non-negative and ordered in the time space of the property owner.
See [interpolation](../../concepts/effects-transitions-and-animation.md#interpolation)
for the available modes and Python/JSON examples.

Animation can be combined with effects and signal bindings. For a signal,
decide whether the signal should replace, add to, or multiply the base and
animated value. For a static edit, set the property directly and leave its
keyframes empty.

Where a setter accepts another property, assignment copies its base value and
keyframes. Both must satisfy the destination's limits; a rejected copy leaves
the destination's value, keyframes and bindings unchanged. Copies between
bindable properties include their signal bindings, including point-component
bindings. Subsequent edits to either property remain independent.

## Effect ownership

You can keyframe an effect before attaching it. `layer.effects.add(effect)`
copies its current values, keyframes and signal bindings, and returns the
attached copy. Later edits to the original do not change that copy; use the
returned handle to edit the attached effect. `project.post_effects.add()` uses
the same copy-on-attachment behavior.

```python
from vestra import GaussianBlur, Project, Rectangle

project = Project(size=(320, 180), fps=30, duration=2)
layer = project.root.add(Rectangle(width=160, height=90, fill="#ffffff"))
template = GaussianBlur(0)
template.radius.keyframe(0, 0)
template.radius.keyframe(1, 8)
attached = layer.effects.add(template)  # includes both keyframes
template.radius.clear_keyframes()     # attached animation stays intact
attached.radius.keyframe(2, 0)         # changes only this layer's copy
```

Common mistakes are putting a child-composition time on the parent timeline
and editing the original effect when you meant to edit its attached copy.
