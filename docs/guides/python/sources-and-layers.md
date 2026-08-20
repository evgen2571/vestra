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

Configure the returned layer for the edit:

```python
layer = project.root.add(Image("assets/photo.png"), duration=5)
layer.opacity.value = 0.85
layer.transform.scale.value = 1.1
layer.transform.position.value = (0.5, 0.45)
```

Use `CompositionLayer` through `composition.group()` for nested content. A
child composition is not an ordinary source in the high-level taxonomy.
