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

The supported geometric inputs are `Rectangle`, `Ellipse`, `Circle`, and
`Polygon`. `Line` remains a visible shape source but is rejected as a mask
input. Image inputs use `alpha` coverage or `luma` coverage. Luma is computed
from encoded RGB values as `0.2126R + 0.7152G + 0.0722B`, then multiplied by
source alpha. Images start at their intrinsic pixel dimensions, centered by the
default normalized `(0.5, 0.5)` position and anchor, before the mask-local
transform is applied. A different-sized image is therefore sampled against its
own intrinsic rectangle; it is not implicitly resized to the layer canvas.
`feather` applies a smooth Gaussian-like coverage blur measured
in output pixels. It supports fractional values, is limited to `0 .. 256` px,
and treats coverage outside the canvas as zero. A value of `0` preserves
hard-mask behavior.
Feathering filters coverage before inversion, operation combination, and
strength interpolation. Image pixels use their intrinsic dimensions before the
mask-local transform; the owning layer transform moves the image mask with the
layer. Text/video inputs, composition masks, and track mattes are deferred.
