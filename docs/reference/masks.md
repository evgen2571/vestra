# Masks reference

The canonical clip field is `masks`, an ordered array of layer-owned mask
objects. Each object has an `id`, a shape `input`, an `operation`, `invert`,
`strength`, and a static `transform`.

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

Only shape inputs are supported in this phase. Feather, dynamic properties,
image alpha/luma inputs, text/video inputs, composition masks, and track mattes
are deferred.
