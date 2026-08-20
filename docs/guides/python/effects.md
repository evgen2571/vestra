# Add effects

Visual effects attach to a layer or to the project output. Layer effects are
processed with that layer. Project `post_effects` apply to the rendered visual
output.

```python
from vestra import Brightness, GaussianBlur
from vestra.sources import Image

layer = project.root.add(Image("assets/photo.png"), duration=5)
layer.effects.add(Brightness(0.1))
blur = layer.effects.add(GaussianBlur(3.0))
blur.radius.keyframe(0, 0.0)
blur.radius.keyframe(1.0, 3.0)

project.post_effects.add(Brightness(0.05))
```

`EffectStack` preserves insertion order. Use layer effects for content-specific
processing and post effects for a whole-project look. Effect parameters that
are exposed as properties can be animated or bound to a signal.

Choose a representative effect first, then validate. Exact parameter ranges
and the complete effect catalog belong to the API reference. An effect does not
change a layer's start or duration.
