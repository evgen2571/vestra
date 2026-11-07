# Masks reference

Use an owned mask when coverage should follow its layer. Use a track matte when
coverage should come from a separately placed sibling layer. See the
[mask guide](../guides/python/masks.md) for placement examples.

| Relationship | Source ownership | Coordinates and timing | API / canonical field |
| --- | --- | --- | --- |
| Owned mask | Copies a source into the consumer's mask collection. | Owner-local coordinates; follows the owning layer and its local lifetime. | `layer.masks.add(source)` / ordered `masks` array. |
| Track matte | References an existing sibling layer. | Composition coordinates; source layer keeps its own timing and presentation. | `layer.set_matte(layer)` / optional `matte` object. |

## Mask fields

| Field | Default | Contract |
| --- | --- | --- |
| `id` | Required in JSON | Non-empty; unique within the owning clip's masks. |
| `input` | Required | Owned shape, image or source; forms below. |
| `operation` | `intersect` | `replace`, `intersect`, `union` or `subtract`. |
| `invert` | `false` | Invert input coverage before combination. |
| `strength` | `1` | Scalar property in `[0, 1]`. |
| `feather` | `0` | Scalar property in `[0, 256]` output pixels. |
| `transform` | Centered identity | Mask-local transform. |

Strength, feather and transform use normal tracks, modifiers and signals.
The Rust canonical types are `vestra_core::project::Mask`, `MaskInput` and
`MaskOperation`, re-exported by the SDK.

## Ordered coverage operations

For accumulated coverage `a` and the current input coverage `m`, both in `[0, 1]`:

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

## Canonical inputs

Schema version 1 supports three `input` forms:

| `input.type` | Fields | Coverage |
| --- | --- | --- |
| `shape` | `geometry`, optional `fill` and `stroke`, as for a shape source | Rendered shape alpha; no `mode` field. |
| `image` | `asset` (image asset ID), `mode` (`alpha` or `luma`) | Intrinsic image pixels. |
| `source` | `source` (a visual source object), `mode` (`alpha` or `luma`) | RGBA rendered by the owned source. |

For example, this mask owns a text source; `mask-font` must reference a font
asset in the project's `assets` array:

```json
{
  "id": "title-mask",
  "input": {
    "type": "source",
    "source": {
      "type": "text",
      "text": "MASK",
      "font": "mask-font",
      "font_size": 48,
      "fill": "#ffffff"
    },
    "mode": "alpha"
  }
}
```

See the complete
[geometric](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/projects/geometric-masks.json) and
[image](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/projects/image-masks.json) examples for other inputs.

## Supported owned sources

Masks consume coverage produced by owned sources. The supported geometric
inputs are `Rectangle`, `Ellipse`, `Circle`, `Polygon`, and `Line`; Line masks
use rendered stroke alpha. Text, Video, Spectrum2D,
ParticleSystem, SolidColor, and Group sources use the same source model when
owned by a mask; an existing timeline Layer is never accepted as a mask.
For non-shapes, `mode=None` selects `alpha`. Explicit invalid modes are
authoring errors; shapes reject any `mode` argument and use rendered alpha.
Luma is computed
from encoded RGB values as `0.2126R + 0.7152G + 0.0722B`, then multiplied by
source alpha.

## Coordinates and intrinsic sizing

Images start at their intrinsic pixel dimensions, centered by the
default normalized `(0.5, 0.5)` position and anchor, before the mask-local
transform is applied. A different-sized image is therefore sampled against its
own intrinsic rectangle; it is not implicitly resized to the layer canvas.
Direct Image and Video `sizing` and `crop` values are not supported in mask
context and are rejected during Python authoring. Use `mask.transform` to
position, scale, or rotate the intrinsic source mask.

## Video mask timing

Video masks start at source time zero and play at normal speed for the owning
layer's duration, independently of that layer's source offset and playback rate.
Media preflight rejects a mask whose authored span exceeds the available video,
including Video children and nested masks inside an owned Group. Video children
retain their own source offsets, playback rates, and child-local durations.

## Owned Group masks

An owned Group is a nested composition with Group-local child timing. Omitted
child duration inherits the containing Group lifetime; explicit child timing
uses the normal half-open interval `[start, start + duration)`. Group children
retain normal Clip presentation fields: transform, effects, opacity, blend mode,
and masks. Child compositing produces the final Group RGBA before alpha/luma
coverage extraction; `mask.transform` then applies to the complete Group result.

## Feathering and evaluation order

Feather applies a smooth Gaussian-like coverage blur in output pixels. It accepts
fractional values in `[0, 256]`, treats coverage outside the canvas as zero, and
preserves hard-mask behavior at `0`.

Coverage is feathered before inversion, ordered operation combination and
strength interpolation. Owned masks apply after layer effects. The result is
intersected with any track matte before final opacity and blending.

## Track Mattes

The consumer clip's optional `matte` field has this canonical representation:

```json
{
  "source_layer": "matte-layer",
  "mode": "alpha",
  "invert": false
}
```

`source_layer` is the ID of a sibling clip, `mode` is required (`alpha` or
`luma`), and `invert` defaults to `false`. Omit `matte` for no relationship.
See the [complete JSON example](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/projects/track-matte.json).

An owned mask accepts a source, rather than an existing timeline layer:

```python
from vestra import Text

lettering = Text("MASK", font_size=48)
portrait.masks.add(lettering)
```

Here `portrait` is an existing layer. To reference a separate sibling layer as a
matte instead, use `set_matte()`; `matte_layer` must already be in that composition:

```python
from vestra import MatteMode

portrait.set_matte(matte_layer, mode=MatteMode.ALPHA)
```

### Ownership and isolated presentation

Track Mattes are limited to layers in the same immediate composition. They
support `Alpha` and `Luma` coverage plus `invert=True`, and a layer can have
one matte. The matte layer is presented in isolation: its source, transform,
effects, owned masks, opacity, and acyclic own matte contribute to coverage;
its outer blend mode is not evaluated against the timeline background.

### Visibility, timing and coordinates

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

### Dependency evaluation and caching

Matte dependencies are evaluated per composition rather than by visual stack
order. A frame-level CPU cache reuses an isolated matte presentation when
several consumers reference the same layer. On WGPU, an already-populated
static-layer cache entry may be reused while rendering an isolated matte
presentation; a cache entry is not shared between consumers for its first
render in a frame. Dynamic dependencies remain frame-local so changing a matte
cannot reuse stale consumer output.
