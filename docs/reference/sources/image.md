# Image source

`Image` draws a still image from a file. Choose sizing to adapt its dimensions
to the composition, and crop to select a region before layer presentation.
See [sources and layers](../../guides/python/sources-and-layers.md) for placement examples.

## API signature

```text title="Source signature (presentation keywords omitted)"
Image(path: str | PathLike[str], *, sizing: Sizing | Literal["original", "fit", "cover"] | None = None,
      crop: Crop | CropProperty | None = None)
```

The constructor also accepts [presentation transform defaults](../../guides/python/sources-and-layers.md),
copied into each placement and individually overridable by `add()`.

## Parameters and sizing

| Parameter | Default | Meaning |
| --- | --- | --- |
| `path` | Required | Non-empty `str` or `PathLike[str]` naming an image file. |
| `sizing` | `None` | Optional sizing override; without one, the containing composition's default applies. |
| `crop` | `None` | Optional `Crop` or mutable `CropProperty`; omission leaves the full source region. |

`Sizing` is immutable and exact:

| Form | Canonical form | Rule |
| --- | --- | --- |
| `Sizing.original()` or `"original"` | `{"mode":"original"}` | Uses intrinsic dimensions. |
| `Sizing.fit()` or `"fit"` | `{"mode":"fit"}` | Fits while preserving aspect ratio. |
| `Sizing.cover()` or `"cover"` | `{"mode":"cover"}` | Covers while preserving aspect ratio. |
| `Sizing.scale(value)` | `{"mode":"scale","scale":value}` | `value` is a finite positive number. |
| `Sizing.stretch(width=1920, height=1080)` | `{"mode":"stretch","width":1920,"height":1080}` | Both are positive integers. |

## Crop coordinates

`Crop(x, y, width, height)` uses **normalized source coordinates**, not pixels.
All four fields must be finite. Semantic validation requires:

- `x >= 0` and `y >= 0`.
- Positive `width` and `height`.
- `x + width <= 1` and `y + height <= 1`.

`crop` becomes a mutable `CropProperty`, so it can be assigned or animated with
the property API. Crop applies while adapting the image, before ordinary layer
transform/compositing. See [coordinates and anchors](../../concepts/coordinates-and-anchors.md)
for the distinction between source and composition coordinates.

## Paths and preparation

Lowering emits an `image` source with an asset reference. Relative paths remain
relative in canonical JSON and resolve against the project base directory during
preflight/preparation. Intrinsic image dimensions come from the prepared asset.

## Placement and presentation

Layer `start`/`duration` place the source on the containing composition's timeline.
Layers own transform, opacity, blend mode, scalar animation, effects and transition
placement. Images support direct transforms and transition endpoints on CPU and WGPU.

## Validation and errors

Construction rejects empty/invalid paths, invalid sizing and an invalid crop
type. `Crop` checks that its fields are finite; the bounds above are semantic
validation rules. Missing or unreadable image files fail preflight, rather than
constructor validation. See [validation layers](../project-format.md#validation-layers)
for the distinction between authoring, canonical validation and media checks.
