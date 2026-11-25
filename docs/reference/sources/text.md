# Text source

`Text` draws titles, labels and captions using a bundled font, an installed
family or a font file. Animate the layer to move or fade the prepared text. See [sources and layers](../../guides/python/sources-and-layers.md)
to add it to a project and export the result.

## API signatures

```text title="Source signature summary (presentation keywords omitted)"
Text(text: str, *, font: str | PathLike[str] | None = None,
     fallback_fonts=(), font_size=48, fill="#ffffff", align="left", max_width=None,
     line_spacing=1.0, letter_spacing=0.0, stroke_color=None, stroke_width=0,
     background_color=None, background_padding=0)
```

```text title="Font discovery signature"
list_system_fonts() -> list[str]
```

The constructor also accepts [presentation transform defaults](../python-api.md#source-presentation),
copied into each placement and individually overridable by `add()`.

## Parameters and layout

| Parameter | Default | Units and accepted values |
| --- | --- | --- |
| `text` | Required | String; empty text is allowed. |
| `font` | `None` | Bundled font, installed family name or file path; see below. |
| `fallback_fonts` | `()` | Ordered sequence of additional family names or file paths. |
| `font_size` | `48` | Pixels; finite and positive. |
| `fill` | `"#ffffff"` | `Color` or a `#RRGGBB` / `#RRGGBBAA` string. |
| `align` | `"left"` | `"left"`, `"center"` or `"right"` within the layout box. |
| `max_width` | `None` | Optional wrapping width in pixels; finite and positive when set. |
| `line_spacing` | `1.0` | Finite positive multiplier; line height is `font_size * line_spacing`. |
| `letter_spacing` | `0.0` | Extra spacing in pixels; finite, including negative values. |

With no `max_width`, each explicit line aligns inside the widest natural line.
With a width, the layout box keeps that width and wraps at words or glyphs.

The constructor canonicalizes colors to hex. Text layout and raster data are
prepared before rendering. Layers own timing, placement, effects and transitions.
Text supports nesting and the same raster presentation on CPU and WGPU.

## Add a stroke and background

| Parameter | Default | Units and accepted values |
| --- | --- | --- |
| `stroke_color` | `None` | Color of the outline outside glyph coverage. Requires a positive `stroke_width`. |
| `stroke_width` | `0` | Finite positive thickness in pixels when a stroke color is supplied. |
| `background_color` | `None` | Color of the box behind the text layout. |
| `background_padding` | `0` | Finite non-negative pixels added on each side of the box. Positive values require a background color. |

Both colors accept `Color` or a `#RRGGBB` / `#RRGGBBAA` string. A padded
background follows the logical layout, including an explicit wrapping width.
Padding rounds outward to the next whole pixel. Painting the stroke and box
can extend the prepared raster past the layout box. The layer's anchor still
uses the layout box, so adding these styles preserves text placement.

## Select a font

| `font` value | Selection |
| --- | --- |
| `None` | Packaged DejaVu-derived VestraTest regular font. |
| Non-empty `str` | Installed family or file, using the [path-disambiguation rules](#family-or-file). |
| `PathLike[str]` | Always a file path. |

The bundled font needs no host setup and provides Latin, Greek, Cyrillic and
other Unicode glyphs. An explicit font file is portable when the same licensed
file is available. Installed families resolve on the machine where a snapshot
is created. The snapshot references the selected files.

Supply `fallback_fonts` for scripts missing from the primary. Each selector
follows the same rules as `font`. Shaping tries the primary first and then the
listed assets in order for missing glyphs. With no fallback list, shaping uses
the primary file alone.

### Family or file

`PathLike` values always mean files. Strings containing `/`, `\\`, or a filename
suffix (such as `Manrope.ttf`) mean files too, as do existing files relative to
`base_directory`. Other strings mean family names. For an extensionless file
that does not yet exist, use `Path("myfont")` or `"./myfont"`. For a family name
containing a dot or slash, discover its file and provide that explicit path.
Relative file paths retain their project base-directory semantics. Missing,
inaccessible or invalid files fail preflight/preparation.

Family matching is exact and case-insensitive, including names with spaces.
Vestra prefers a regular face and breaks ties by file path and collection index.
An unavailable family raises `ValueError` during snapshot creation; it never
silently substitutes another family.

### Discover installed families

This independent lookup needs no project:

```python
from vestra import list_system_fonts

print(list_system_fonts())
```

Use `Text("Hello", font="DejaVu Sans")` only if that family appears in the list.
`list_system_fonts()` returns unique installed families, sorted without regard
to case. Empty discovery returns `[]`; no external font utilities are needed.

| Platform | Discovery and limits |
| --- | --- |
| Linux | Uses fontconfig configuration, including `FONTCONFIG_FILE` and configured NixOS directories, with standard-directory fallback. Fontconfig aliases and substitution rules do not replace exact matching. |
| Windows | Scans standard system/user font directories. Application-private fonts, registry-only custom locations and uninstalled cloud fonts may not appear. |
| macOS | Scans standard system/user font directories. Application-private fonts, resource-fork/dfont files and uninstalled cloud fonts may not appear. |

### Discovery and font-collection caches

Discovery is cached for the process: restart Python after installing fonts or
changing font configuration. Collections use the chosen face through a standalone
file in the user cache (`vestra/fonts` under the platform cache directory;
`XDG_CACHE_HOME` on Linux). Keep that file available for saved snapshots, or
restart Python and recreate the snapshot after clearing caches. The default
font is bundled in the package and does not need a writable cache.

## Validation and error timing

| Stage | Failure |
| --- | --- |
| Construction | Invalid types, empty font selectors, invalid colors, or invalid layout or style values. |
| Snapshot creation | An unavailable primary or fallback family raises `ValueError`. |
| Preflight/preparation | A selected font file must exist and load successfully. |

## Canonical JSON

The source tag is `text`. In a project snapshot, `font` and each entry in
`fallback_fonts` are registered **Font asset IDs**. The source model uses the same
layout and style field names shown above. See [project format](../project-format.md)
for the surrounding asset and clip objects.
