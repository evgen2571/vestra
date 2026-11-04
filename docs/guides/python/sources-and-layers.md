# Sources and layers

A source describes visual content. Adding it to a composition creates a layer
that supplies timing and presentation.

```python
from vestra.sources import Color, Image, Rectangle, Text, Video

project.root.add(Image("assets/photo.png"), start=0, duration=5, id="photo")
project.root.add(Video("assets/clip.mp4"), start=1, duration=3, id="clip")
project.root.add(
    Rectangle(width=600, height=180, fill="#0f172a"),
    start=2,
    duration=2,
    z=2,
    id="panel",
)
```

Other current source families include `Color`/`SolidColor`, `Text`,
`Spectrum2D`, and `ParticleSystem`. Shapes are represented by concrete
classes such as `Rectangle`, `Ellipse`, `Circle`, `Line`, and `Polygon`.
`Text` requires an explicit font file. Image and video sources can carry
sizing and crop properties.

`Color`/`SolidColor` fills the canvas in canonical JSON, so it has no direct
canonical transform or transition endpoint. In Python, its layer can still be
transformed or used in a transition. The authoring API supplies the needed
presentation wrapper when it lowers the project.

Configure the returned layer for the edit:

```python
layer = project.root.add(Image("assets/photo.png"), duration=5)
layer.opacity.value = 0.85
layer.transform.scale.value = 1.1
layer.transform.position.value = (0.5, 0.45)
```

Use a reusable `Group` source or author a `CompositionLayer` directly through
`composition.group()` for [nested content](nested-compositions.md).

Visual sources accept `position`, `anchor`, `scale`, and `rotation` (alias
`rotation_degrees`) as constructor defaults. For example:

```python
panel = Rectangle(width=200, height=100, fill="#ff0000",
                  position=(0.4, 0.5), rotation=15)
first = project.root.add(panel)
second = project.root.add(panel, position=(0.75, 0.5))
second.transform.rotation.keyframe(0, 15)
second.transform.rotation.keyframe(2, 180)
```

These values initialize the placement's existing transform; they add no extra
rendering pass. Position is normalized to the parent canvas, anchor is within
`[0, 1]`, scale is a positive number or pair, and rotation is finite degrees.
Defaults remain position/anchor `(0.5, 0.5)`, scale `(1, 1)`, rotation `0`.
ParticleSystem's existing `rotation` controls particle orientation; use
`rotation_degrees` for its presentation default.

You can configure `source.transform` before placement, including keyframes and
component audio bindings, or pass `transform=my_transform` to a constructor or
`add()`. Every placement copies all transform state. A supplied `transform`
replaces the entire default transform and cannot be mixed with individual
transform arguments. Supplying both rotation spellings is also an error.

Individual `add()` overrides replace the selected property's value, keyframes,
and bindings, including its X/Y component bindings. Unspecified properties
retain their complete state. Later edits through `layer.transform` use the
normal property behavior; assigning a numeric `.value` preserves its existing
keyframes and bindings. Later source edits affect only future placements.

Scale bindings apply in order: uniform `transform.scale` bindings, then
`transform.scale.x`/`.y`, then `transform.scale_x`/`scale_y` bindings.
