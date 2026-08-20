# Text source

`Text(text, *, font, font_size, fill="#ffffff", align="left", max_width=None,
line_spacing=1.0, letter_spacing=0.0)` creates a static file-backed text source.
`text` is a string. `font` is a non-empty string or `PathLike` file path. The
constructor canonicalizes `fill` to `#RRGGBB` or `#RRGGBBAA`.

`font_size`, `max_width` when set, and `line_spacing` are finite and positive.
`letter_spacing` is finite and can be negative. `align` is `left`, `center`, or
`right`. Line height is `font_size * line_spacing`. The canonical tag is `text`
with `text`, `font`, `font_size`, `fill`, `align`, optional `max_width`,
`line_spacing`, and `letter_spacing`.

Text layout/raster data is preparation work, not JSON data. The font must exist
and load successfully during preflight/preparation. Layer transform, animation,
effects, transitions, nesting, CPU, and WGPU support follow the ordinary text
source path, provided that font preparation succeeds.
