# Geometric masks

A mask belongs to a layer. It is separate from the layer's effects and is
applied after effects, before final opacity and blending.

```python
import vestra

project = vestra.Project(size=(1920, 1080), fps=30, duration=5)
layer = project.root.add(vestra.Image("character.png"), duration=5)
layer.masks.add(vestra.Circle(radius=300, fill="#ffffffff"))
hole = layer.masks.add(
    vestra.Circle(radius=120, fill="#ffffffff"),
    operation=vestra.MaskOperation.SUBTRACT,
)
hole.invert = False
```

`layer.masks.items` preserves declaration order. Use `remove()` or `clear()`
to manage the collection. Supported inputs are `Rectangle`, `Ellipse`,
`Circle`, and `Polygon` shape objects. Shapes still need a fill or stroke,
just as they do when used as visible sources.
`Line` remains available as a visible shape source but is not a supported mask
input in this phase.

Coverage starts at `1`. The default operation is `INTERSECT`, so one ordinary
mask reveals the part of the layer covered by its shape. `REPLACE`, `UNION`,
and `SUBTRACT` use normalized coverage and can be combined in order. `invert`
changes coverage to `1 - coverage`. `strength=0` leaves the accumulated
coverage unchanged; `strength=1` applies the selected operation completely.

Mask transforms use the existing static transform property and are local to
the owning layer. The layer transform moves the layer and its attached mask
together. Effects run before masks, and layer opacity runs after masks.
Groups are maskable layers, so a group mask clips the already-composited group
result.

This release supports static geometric masks only. Feathering, animated or
signal-driven masks, image masks, composition masks, and track mattes are not
part of this feature.
