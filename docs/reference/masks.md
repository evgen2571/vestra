# Masks reference

The canonical clip field is `masks`, an ordered array of layer-owned mask
objects. Each object has an `id`, a shape `input`, an `operation`, `invert`,
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
input. `feather` is a non-negative Gaussian-like coverage radius in output
pixels. It uses nine taps per separable pass, with sigma equal to `feather / 3`,
clamps at `256` output pixels, preserves fractional values, and treats samples
outside the canvas as zero. A value of `0` preserves hard-mask behavior.
Feathering filters coverage before inversion, operation combination, and
strength interpolation. Image alpha/luma inputs, text/video inputs,
composition masks, and track mattes are deferred.
