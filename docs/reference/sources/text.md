# Text source

`Text` draws static text using a bundled font, an installed family or a font file.
Use it for titles, labels and paragraphs; animate the layer to move or fade the
prepared text. See [sources and layers](../../guides/python/sources-and-layers.md)
for a complete placement example and font-portability guidance.

## API signatures

```text title="Source signature (presentation keywords omitted)"
Text(text: str, *, font: str | PathLike[str] | None = None,
     font_size=48, fill="#ffffff", align="left", max_width=None,
     line_spacing=1.0, letter_spacing=0.0)
```

```text title="Font discovery signature"
list_system_fonts() -> list[str]
```

The constructor also accepts [presentation transform defaults](../../guides/python/sources-and-layers.md),
copied into each placement and individually overridable by `add()`.

## Parameters and layout

| Parameter | Default | Units and accepted values |
| --- | --- | --- |
| `text` | Required | String; empty text is allowed. |
| `font` | `None` | Bundled font, installed family name or file path; see below. |
| `font_size` | `48` | Pixels; finite and positive. |
| `fill` | `"#ffffff"` | `Color` or a `#RRGGBB` / `#RRGGBBAA` string. |
| `align` | `"left"` | `"left"`, `"center"` or `"right"` within the layout box. |
| `max_width` | `None` | Optional wrapping width in pixels; finite and positive when set. |
| `line_spacing` | `1.0` | Finite positive multiplier; line height is `font_size * line_spacing`. |
| `letter_spacing` | `0.0` | Extra spacing in pixels; finite, including negative values. |

The constructor canonicalizes `fill` to a hex color. Layout and raster data are
created during preparation; they are not stored in JSON. Layers own placement
timing, transforms, animation, effects and transitions. Text supports nesting
and the ordinary CPU/WGPU raster presentation path once font preparation succeeds.

## Select a font

| `font` value | Selection |
| --- | --- |
| `None` | Packaged DejaVu-derived VestraTest regular font. |
| Non-empty `str` | Installed family or file, using the [path-disambiguation rules](../../guides/python/sources-and-layers.md). |
| `PathLike[str]` | Always a file path. |

`list_system_fonts()` returns unique installed families, sorted without regard
to case and cached for the process. The guide covers platform discovery and
font-collection caching.

## Validation and error timing

| Stage | Failure |
| --- | --- |
| Construction | Invalid text/font types, empty font selector, invalid color, alignment or numeric layout values. |
| Snapshot creation | An unavailable installed family raises `ValueError`; there is no fallback. |
| Preflight/preparation | A selected font file must exist and load successfully. |

## Canonical JSON

The source tag is `text`. Fields are `text`, `font`, `font_size`, `fill`, `align`,
optional `max_width`, `line_spacing` and `letter_spacing`. In a project snapshot,
`font` is a registered **Font asset ID**, rather than the Python selector.
See [project format](../project-format.md) for the surrounding asset and clip objects.
