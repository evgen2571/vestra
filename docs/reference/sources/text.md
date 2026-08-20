# Text source

`vestra.sources.Text(text, *, font, font_size, fill="#ffffff", align="left",
max_width=None, line_spacing=1.0, letter_spacing=0.0)` creates a text source.
`align` is `left`, `center`, or `right`. `font` is an explicit non-empty font
file path. `font_size`, `max_width` when set, and `line_spacing` are positive;
`letter_spacing` is finite. The canonical tag is `text`.

Text supports fill, layer transform, animation, effects, and transitions. A
font must exist and be usable during preflight or render preparation. CPU and
WGPU text support depends on successful font loading.
