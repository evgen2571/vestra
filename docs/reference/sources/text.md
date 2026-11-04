# Text source

`Text(text, *, font=None, font_size=48, fill="#ffffff", align="left", max_width=None,
line_spacing=1.0, letter_spacing=0.0)` creates a static file-backed text source.
`text` is a string. `font=None` uses the packaged DejaVu-derived VestraTest
regular font. A non-empty string can select an installed family or file;
`PathLike[str]` always selects a file. `list_system_fonts() -> list[str]` returns
case-insensitively sorted unique installed families, cached for the process. An unavailable family
raises `ValueError` at snapshot creation, without fallback. See
[font selection and portability](../../guides/python/sources-and-layers.md)
for path disambiguation, platform discovery and collection-cache behavior.
The constructor canonicalizes `fill` to `#RRGGBB` or `#RRGGBBAA`.

`font_size` defaults to 48 pixels. `font_size`, `max_width` when set, and
`line_spacing` are finite and positive.
`letter_spacing` is finite and can be negative. `align` is `left`, `center`, or
`right`. Line height is `font_size * line_spacing`. The canonical tag is `text`
with `text`, `font` (a registered Font asset ID), `font_size`, `fill`, `align`, optional `max_width`,
`line_spacing`, and `letter_spacing`.

Text layout/raster data is preparation work, not JSON data. The font must exist
and load successfully during preflight/preparation. Layer transform, animation,
effects, transitions, nesting, CPU, and WGPU support follow the ordinary text
source path, provided that font preparation succeeds.

Python constructors also accept [presentation transform defaults](../../guides/python/sources-and-layers.md),
which are copied into each placement and can be individually overridden by `add()`.
