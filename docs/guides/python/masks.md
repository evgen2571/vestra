# Layer masks

A mask reveals part of a layer without changing its source image. Start with a
circular portrait crop, then add a soft reveal and a cutout one step at a time.
Use your own `character.png` in the directory where you run the script.

## Start with a circular crop

```python
from vestra import Circle, Image, Project

project = Project(
    size=(1920, 1080),
    fps=30,
    duration=5,
)

photo = Image("character.png")
portrait = project.root.add(photo)

circle = Circle(radius=300, fill="#ffffff")
window = portrait.masks.add(circle)
```

The portrait inherits the project's five-second duration. The circle is centered
on the layer and its radius is in pixels. Its white fill supplies opaque
coverage: you see the photo inside the circle, not a white circle over it.
The layer's transform moves the photo and its mask together.

## Animate a soft reveal

Continue with the same `window`; there is no need to add another circle:

```python
window.feather = 20
window.transform.scale.keyframe(0, 0.2)
window.transform.scale.keyframe(2, 1)
```

The window grows from 20% to full size over two seconds, then holds its size.
These times are measured from the portrait layer's start. Feathering softens the
edge over 20 output pixels; it does not blur the photograph itself.

## Cut a hole out of the portrait

Add a smaller circle with a subtract operation:

```python
from vestra import MaskOperation

hole = Circle(radius=120, fill="#ffffff")
portrait.masks.add(hole, operation=MaskOperation.SUBTRACT)
```

Masks combine in declaration order. The first mask reveals a circular window;
the second removes its center. The hole stays fixed while the outer window grows,
so the result is an expanding ring. Leave out this step when you want a solid portrait.

## Render the result

```python
project.render("portrait-reveal.mp4", backend="cpu", overwrite=True)
```

This writes a five-second, silent MP4 to the current directory. Rerunning it
replaces that file. To try the alternatives below, replace the masks before
rendering again.

## Use an image as a mask

A grayscale image can supply coverage through its brightness. Here `gradient.png`
is a separate image: bright areas reveal the portrait and dark areas hide it.

```python
from vestra import MaskCoverageMode

portrait.masks.clear()
gradient = Image("gradient.png")
portrait.masks.add(gradient, mode=MaskCoverageMode.LUMA, feather=12)
```

To use an image's transparency instead, replace that mask with an alpha mask:

```python
portrait.masks.clear()
alpha_image = Image("alpha.png")
image_mask = portrait.masks.add(alpha_image, feather=8)
image_mask.transform.scale = 0.75
```

Alpha coverage is the default for image masks. Images keep their intrinsic pixel
size; use the mask transform to align a differently sized mask with the photo.
The mask image and the portrait do not need to be the same file or dimensions.

## Use text or grouped content

A mask can own a normal visual source. For example, reveal the photo through
letters using the bundled font:

```python
from vestra import Text

portrait.masks.clear()
lettering = Text("Vestra", font_size=96)
portrait.masks.add(lettering)
```

For a composite mask, put its parts in a `Group`. This example combines a masked
rectangle and a text label before turning their rendered alpha into coverage:

```python
from vestra import Group, Rectangle

portrait.masks.clear()
mask_artwork = Group()

backplate = mask_artwork.add(Rectangle(width=900, height=600, fill="#ffffff"))
backplate.masks.add(Circle(radius=240, fill="#ffffff"))
mask_artwork.add(Text("Vestra", font_size=96))

portrait.masks.add(mask_artwork)
```

Video, spectrum and particle sources can also provide coverage. Video masks play
from source time zero; spectrum masks need audio on the project timeline. See the
[masks reference](../../reference/masks.md) for supported sources and timing rules.

## Coverage, timing and ordering

`portrait.masks.items` preserves declaration order. Use `remove()` to remove one
mask or `clear()` to start again. Geometric inputs include `Rectangle`, `Ellipse`,
`Circle`, `Polygon` and `Line`; a shape needs a fill or stroke. Lines use their
rendered stroke alpha, including antialiased edges.

`MaskCoverageMode` selects alpha or luma coverage. These are owned source masks,
not references to another timeline Layer.

`Group` is an owned nested visual composition. Child timing is local to the
Group: omitted `duration` inherits the containing Group lifetime, while an
explicit `start` and `duration` clip the child to that local interval. Children
also expose the normal presentation fields (`transform`, `effects`, `opacity`,
`blend_mode`, and `masks`). The Group is composited normally before its final
RGBA is converted to alpha or luma coverage. `mask.transform` is applied after
that Group rendering, so it moves the complete Group coverage rather than an
individual child.

### Alpha and brightness

`ALPHA` uses source alpha without thresholding. `LUMA` uses encoded RGB values
with Rec.709 coefficients (`0.2126R + 0.7152G + 0.0722B`) multiplied by
source alpha. Image pixels use their intrinsic dimensions before the
mask-local transform. Direct Image and Video `sizing` and `crop` settings are
rejected for mask inputs. Use `mask.transform` for placement and scale; Video
timing remains owner-layer-local. Group child placements retain their normal
clip sizing and crop fields.

### Combine masks

Coverage starts at `1`. The default operation is `INTERSECT`, so one ordinary
mask reveals the part of the layer covered by its shape. `REPLACE`, `UNION`,
and `SUBTRACT` use normalized coverage and can be combined in order. `invert`
changes coverage to `1 - coverage`. `strength=0` leaves the accumulated
coverage unchanged; `strength=1` applies the selected operation completely.

### Animate mask properties

`strength` and `feather` are scalar properties and support the normal
keyframes, modifiers, and signal bindings. Mask transforms use the existing
transform properties and are local to the owning layer. Uniform bindings such
as `mask.transform.scale.bind(signal)` lower to the same `scale_x` and
`scale_y` component modifiers used by layer transforms. The layer transform
moves the layer and its attached mask together. Effects run before masks, and
layer opacity runs after masks.
Groups are maskable layers, so a group mask clips the already-composited group
result.

### Feathering and source defaults

Feather applies a smooth Gaussian-like coverage blur measured in output
pixels. Values from `0` through `256` px are supported, fractional values are
kept, and samples outside the canvas have zero coverage. It filters coverage
before inversion, operation combination, and strength interpolation. Static
masks remain cacheable; animated, modifier-driven, or signal-driven mask
properties make the owning layer dynamic. Masks attach to layers, including
group layers; a composition has no mask stack. For coverage from another layer,
use a [track matte](../../reference/masks.md#track-mattes).

A mask placement also copies its source's constructor transform defaults.
Edit `mask.transform` afterward; defaults are applied once and are independent
of later source edits.
