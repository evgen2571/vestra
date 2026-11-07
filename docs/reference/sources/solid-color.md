# Solid-color source

`Color` fills the layer surface with one color. Use it for backgrounds or solid
panels. `SolidColor` is an alias for the same high-level class.

## API signature

```text title="Source signature (presentation keywords omitted)"
Color(value="#000000")
SolidColor(value="#000000")
```

`value` accepts an authoring `Color` value or a `#RRGGBB` / `#RRGGBBAA` string.
The default is opaque black; an eight-digit value includes alpha. Invalid colors
fail during construction or assignment to `value`.

## Extent and Python presentation

The containing canvas supplies the source extent. There is no intrinsic asset
size, crop, source trim or media preparation. Layers own timing, effects,
opacity and blending.

High-level Python layers can use transforms and transitions with this source.
Vestra lowers that presentation through an adapter. The constructor accepts
[presentation transform defaults](../../guides/python/sources-and-layers.md),
copied into each placement and individually overridable by `add()`.

## Canonical JSON restrictions

The serialized source is `{"type":"solid_color","colour":"#RRGGBB"}`
(or an eight-digit color). A canonical `solid_color` clip cannot carry a
`transform` and cannot be named directly in a transition placement. Hand-authored
JSON must respect those restrictions even though Python can adapt presentation.

Solid-color rendering is supported on CPU and WGPU, including nested local
canvases. See [project format](../project-format.md) and the
[feature matrix](../feature-support.md) for the surrounding contracts.
