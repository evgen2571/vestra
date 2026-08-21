# Masks reference

The canonical clip field is `masks`, an ordered array of layer-owned mask
objects. Each object has an `id`, an input, an `operation`, `invert`,
scalar properties `strength` and `feather`, and a transform. These properties
use the normal track, modifier, and signal machinery.

The Rust canonical types are `vestra_core::project::Mask`, `MaskInput`, and
`MaskOperation`; the SDK crate re-exports these types.

The normalized coverage rules are:

```text
initial = 1
replace   = m
intersect = a * m
union     = a + m - a*m
subtract  = a * (1 - m)
out       = lerp(a, operation(a, m), strength)
```

Inversion happens before the operation. The supported operations are
`replace`, `intersect`, `union`, and `subtract`. The default is `intersect`.

Schema version 4 is the current emitted project format. Vestra accepts valid
schema version 3 projects and treats their omitted masks as an empty list
before normalizing the in-memory project to version 4.
Vestra keeps schema 4 for Masks v1 because the 0.1 project language is still
evolving and has not become a stable external compatibility boundary. Image
mask inputs therefore do not trigger a version bump; schema 3 loading remains
supported.

Masks consume coverage produced by owned sources. The supported geometric
inputs are `Rectangle`, `Ellipse`, `Circle`, `Polygon`, and `Line`; Line masks
use rendered stroke alpha. Text, Video, Spectrum2D,
ParticleSystem, SolidColor, and Group sources use the same source model when
owned by a mask; an existing timeline Layer is never accepted as a mask.
Sources use `alpha` coverage or `luma` coverage. Luma is computed
from encoded RGB values as `0.2126R + 0.7152G + 0.0722B`, then multiplied by
source alpha. Images start at their intrinsic pixel dimensions, centered by the
default normalized `(0.5, 0.5)` position and anchor, before the mask-local
transform is applied. A different-sized image is therefore sampled against its
own intrinsic rectangle; it is not implicitly resized to the layer canvas.
Direct Image and Video `sizing` and `crop` values are not supported in mask
context and are rejected during Python authoring. Use `mask.transform` to
position, scale, or rotate the intrinsic source mask. Video frame timing still
follows the owning layer's local time.
An owned Group is a nested composition with Group-local child timing. Omitted
child duration inherits the containing Group lifetime; explicit child timing
uses the normal half-open interval `[start, start + duration)`. Group children
retain normal Clip presentation fields: transform, effects, opacity, blend mode,
and masks. Child compositing produces the final Group RGBA before alpha/luma
coverage extraction; `mask.transform` then applies to the complete Group result.
`mode=None` selects the source-appropriate default (`alpha`). Explicit invalid
modes are authoring errors. `feather` applies a smooth Gaussian-like coverage blur measured
in output pixels. It supports fractional values, is limited to `0 .. 256` px,
and treats coverage outside the canvas as zero. A value of `0` preserves
hard-mask behavior.
Feathering filters coverage before inversion, operation combination, and
strength interpolation. Image pixels use their intrinsic dimensions before the
mask-local transform; the owning layer transform moves the image mask with the
layer. An owned Group is rendered as a self-contained precomposition before
coverage extraction.

## Track Mattes

A Track Matte references existing timeline content; it does not own a new
source. The two relationships are intentionally distinct:

```python
layer.masks.add(vestra.Text("MASK", font="font.ttf"))  # owned source
layer.set_matte(matte_layer, mode=vestra.MatteMode.ALPHA)  # layer reference
```

Track Mattes are limited to layers in the same immediate composition. They
support `Alpha` and `Luma` coverage plus `invert=True`, and a layer can have
one matte. The matte layer is presented in isolation: its source, transform,
effects, owned masks, opacity, and acyclic own matte contribute to coverage;
its outer blend mode is not evaluated against the timeline background.

The matte layer keeps its normal visibility and timing. A hidden layer can
still provide matte coverage, while a visible layer is both composited at its
normal stack position and usable as a matte. Outside the matte layer's own
active interval, its raw coverage is zero: a normal matte hides its consumer,
while an inverted matte leaves the consumer's existing coverage unchanged.
Matte activation or deactivation within a consumer's lifetime therefore makes
an otherwise static consumer temporally dynamic. Matte coordinates are
composition-space layer coordinates: moving the consumer does not move the
matte. Owned masks instead follow their owning layer and use owner-local source
timing.

The consumer applies owned masks first, intersects their result with matte
coverage, then applies final opacity and blend/compositing. Cycles, self
references, missing sources, and cross-composition references are rejected
during validation.

Matte dependencies are evaluated per composition rather than by visual stack
order. A frame-level CPU cache reuses an isolated matte presentation when
several consumers reference the same layer. On WGPU, an already-populated
static-layer cache entry may be reused while rendering an isolated matte
presentation; a cache entry is not shared between consumers for its first
render in a frame. Dynamic dependencies remain frame-local so changing a matte
cannot reuse stale consumer output.
