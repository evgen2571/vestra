# Layer masks

A mask belongs to a layer. It is separate from the layer's effects and is
applied after effects, before final opacity and blending.

```python
import vestra

project = vestra.Project(size=(1920, 1080), fps=30, duration=5)
layer = project.root.add(vestra.Image("character.png"), duration=5)
layer.masks.add(vestra.Circle(radius=300, fill="#ffffffff"))
reveal = layer.masks.add(
    vestra.Circle(radius=300, fill="#ffffffff"),
    feather=20,
)
reveal.transform.scale.keyframe(0, (0.2, 0.2))
reveal.transform.scale.keyframe(2, (1.0, 1.0))
hole = layer.masks.add(
    vestra.Circle(radius=120, fill="#ffffffff"),
    operation=vestra.MaskOperation.SUBTRACT,
)
hole.invert = False
```

`layer.masks.items` preserves declaration order. Use `remove()` or `clear()`
to manage the collection. Supported inputs include `Rectangle`, `Ellipse`,
`Circle`, `Polygon`, and `Line` shape objects. Lines use rendered stroke alpha,
including antialiased edges. Shapes still need a fill or stroke, just as they
do when used as visible sources.

The normal `Image` source can also provide static image coverage:

```python
layer.masks.add(
    vestra.Image("gradient.png"),
    mode=vestra.ImageMaskMode.LUMA,
    feather=12,
)
image_mask = layer.masks.add(
    vestra.Image("alpha.png"),
    mode=vestra.ImageMaskMode.ALPHA,
    feather=8,
)
image_mask.transform.scale = (0.75, 0.75)
```

Any supported self-contained source can be owned by a mask. The source is
rendered normally, then its RGBA result is converted to coverage:

```python
layer.masks.add(vestra.Text("VESTRA", font="VestraTest-Regular.ttf", font_size=96))
layer.masks.add(vestra.Video("matte.mp4"), mode=vestra.MaskCoverageMode.LUMA)
layer.masks.add(vestra.Spectrum2D())
layer.masks.add(vestra.ParticleSystem())
layer.masks.add(vestra.Group([
    vestra.Rectangle(width=900, height=600, fill="#ffffffff"),
    vestra.Text("VESTRA", font="VestraTest-Regular.ttf", font_size=96),
]))

owned_group = vestra.Group()
child = owned_group.add(vestra.Rectangle(width=900, height=600, fill="#ffffffff"))
child.masks.add(vestra.Circle(radius=240, fill="#ffffffff"))
layer.masks.add(owned_group)
```

`MaskCoverageMode` is the generic name; `ImageMaskMode` remains a compatibility
alias. These are owned source masks, not references to another timeline Layer.

`Group` is an owned nested visual composition. Child timing is local to the
Group: omitted `duration` inherits the containing Group lifetime, while an
explicit `start` and `duration` clip the child to that local interval. Children
also expose the normal presentation fields (`transform`, `effects`, `opacity`,
`blend_mode`, and `masks`). The Group is composited normally before its final
RGBA is converted to alpha or luma coverage. `mask.transform` is applied after
that Group rendering, so it moves the complete Group coverage rather than an
individual child.

`ALPHA` uses source alpha without thresholding. `LUMA` uses encoded RGB values
with Rec.709 coefficients (`0.2126R + 0.7152G + 0.0722B`) multiplied by
source alpha. Image pixels use their intrinsic dimensions before the
mask-local transform. Direct Image and Video `sizing` and `crop` settings are
rejected for mask inputs. Use `mask.transform` for placement and scale; Video
timing remains owner-layer-local. Group child placements retain their normal
clip sizing and crop fields.

Coverage starts at `1`. The default operation is `INTERSECT`, so one ordinary
mask reveals the part of the layer covered by its shape. `REPLACE`, `UNION`,
and `SUBTRACT` use normalized coverage and can be combined in order. `invert`
changes coverage to `1 - coverage`. `strength=0` leaves the accumulated
coverage unchanged; `strength=1` applies the selected operation completely.

`strength` and `feather` are scalar properties and support the normal
keyframes, modifiers, and signal bindings. Mask transforms use the existing
transform properties and are local to the owning layer. Uniform bindings such
as `mask.transform.scale.bind(signal)` lower to the same `scale_x` and
`scale_y` component modifiers used by layer transforms. The layer transform
moves the layer and its attached mask together. Effects run before masks, and
layer opacity runs after masks.
Groups are maskable layers, so a group mask clips the already-composited group
result.

Feather applies a smooth Gaussian-like coverage blur measured in output
pixels. Values from `0` through `256` px are supported, fractional values are
kept, and samples outside the canvas have zero coverage. It filters coverage
before inversion, operation combination, and strength interpolation. Static
masks remain cacheable; animated, modifier-driven, or signal-driven mask
properties make the owning layer dynamic. Composition masks and track mattes
are not supported.
