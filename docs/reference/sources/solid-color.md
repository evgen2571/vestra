# Solid-color source

`Color(value)` is the high-level class. `SolidColor` is the same class, kept as
an alias. `value` is an authoring `Color` or a canonical `#RRGGBB` or
`#RRGGBBAA` string. Alpha is part of the colour value when supplied. The
serialized source is `{"type":"solid_color","colour":"..."}`.

Solid colour fills the layer surface. Output canvas size supplies its extent;
the source has no intrinsic asset size, crop, or source trim. Layer timing,
transform, opacity animation, effects, and transitions remain layer contracts.
It supports canonical JSON, high-level Python, CPU, WGPU, nesting, and direct
transition endpoints.
