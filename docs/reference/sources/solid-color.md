# Solid-color source

`Color(value)` is the high-level class. `SolidColor` is the same class, kept as
an alias. `value` is an authoring `Color` or a canonical `#RRGGBB` or
`#RRGGBBAA` string. Alpha is part of the colour value when supplied. The
serialized source is `{"type":"solid_color","colour":"#RRGGBB"}`.

Solid colour fills the layer surface. Output canvas size supplies its extent;
the source has no intrinsic asset size, crop, source trim, or canonical
`transform`. It is not a direct canonical transition endpoint.

High-level Python layers may still use a transform or a transition with
`Color`/`SolidColor`. Vestra applies that presentation through an adapter when
it lowers the high-level project. Hand-authored canonical JSON must keep the
`solid_color` clip itself free of `transform` and cannot name it directly in a
transition placement.
