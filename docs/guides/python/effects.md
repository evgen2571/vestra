# Add effects

Visual effects attach to a layer or to the project output. Layer effects are
processed with that layer. Project `post_effects` apply to the rendered visual
output.

```python
from vestra import Brightness, GaussianBlur
from vestra.sources import Image

layer = project.root.add(Image("assets/photo.png"), duration=5)
layer.effects.add(Brightness(0.1))
# Keep the attached effect handle so its radius can be animated.
blur = layer.effects.add(GaussianBlur(3.0))
blur.radius.keyframe(0, 0.0)
blur.radius.keyframe(1.0, 3.0)

# Post effects process the final composition, after its layers.
project.post_effects.add(Brightness(0.05))
```

Motion Tile is intended for the common edge-filling edit where a layer is
zoomed, rotated, or shaken:

```python
from vestra import ChromaticAberration, DirectionalBlur, MotionTile

layer.effects.add(MotionTile(200, 200, mirror_edges=True))
layer.transform.scale = (1.25, 1.25)
layer.effects.add(DirectionalBlur(12, 45))
layer.effects.add(ChromaticAberration(4, 0))
```

Motion Tile runs before the layer transform. Directional Blur, Radial Blur,
Chromatic Aberration, and the other image effects run after it. Its percentage
parameters describe the virtual tiled source region, while the composition
canvas size is unchanged.

`EffectStack` preserves insertion order. Use layer effects for content-specific
processing and post effects for a whole-project look. Effect parameters that
are exposed as properties can be animated or bound to a signal.

Choose a representative effect first, then validate. Exact parameter ranges
and the complete effect catalog belong to the API reference. An effect does not
change a layer's start or duration.
