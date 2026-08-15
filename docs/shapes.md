# Primitive Shape Sources

Vestra supports static, source-local procedural Shapes as ordinary visual
Sources. Their geometry and style are fixed for v1B; position, scale, rotation,
anchor, opacity, effects, blends, Groups, and transitions remain Layer-owned.

```python
from vestra import Circle, Project, Rectangle

project = Project(size=(1920, 1080), fps=30, duration=5)
scene = project.root

scene.add(Rectangle(width=1920, height=1080, fill="#111111"))
badge = scene.add(
    Circle(radius=120, fill="#ffffff", stroke="#ff3355", stroke_width=8)
)
badge.transform.position.value = (0.5, 0.5)
```

The other public primitives are `Rectangle` with an optional `corner_radius`,
`Ellipse`, `Line`, and `Polygon`. Shapes are rasterized once during preparation into the shared
generic raster Source path; Layer transforms and effects are evaluated per
frame.
