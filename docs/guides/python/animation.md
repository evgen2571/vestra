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

Animation can be combined with effects and signal bindings. For a signal,
decide whether the signal should replace, add to, or multiply the base and
animated value. For a static edit, set the property directly and leave its
keyframes empty.

Common mistakes are putting a child-composition time on the parent timeline,
using a source object where a layer property is required, and keyframing an
effect before adding that effect to its layer.
