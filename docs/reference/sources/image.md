# Image source

```python
Image(path: str | PathLike[str], *, sizing: Sizing | Literal["original", "fit", "cover"] | None = None,
      crop: Crop | CropProperty | None = None)
```

`path` must be a non-empty `str` or `PathLike[str]`. Lowering emits an `image` source with an asset reference. Relative paths remain relative in canonical JSON and resolve against the project base directory during preflight/preparation. Intrinsic image dimensions come from the prepared asset.

`Sizing` is immutable and exact:

| Form | Canonical form | Rule |
| --- | --- | --- |
| `Sizing.original()` or `"original"` | `{"mode":"original"}` | Uses intrinsic dimensions. |
| `Sizing.fit()` or `"fit"` | `{"mode":"fit"}` | Fits while preserving aspect ratio. |
| `Sizing.cover()` or `"cover"` | `{"mode":"cover"}` | Covers while preserving aspect ratio. |
| `Sizing.scale(value)` | `{"mode":"scale","scale":value}` | `value` is a finite positive number. |
| `Sizing.stretch(width=1920, height=1080)` | `{"mode":"stretch","width":1920,"height":1080}` | Both are positive integers. |

`Crop(x, y, width, height)` has four finite numeric fields. It uses normalized source coordinates: semantic validation requires `x >= 0`, `y >= 0`, positive `width`/`height`, `x + width <= 1`, and `y + height <= 1`. `crop` becomes a mutable `CropProperty`, so it can be assigned or animated with the property API. Crop applies while adapting the image before ordinary layer transform/compositing.

Layer `start`/`duration` place the source on the project timeline. Layers own transform, opacity, blend mode, scalar animation, effects and transition placement. Images support direct transforms and transition endpoints. Missing/unreadable files fail preflight, not constructor validation.
