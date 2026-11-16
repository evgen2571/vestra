# Text source

`Text` draws static text using a bundled font, an installed family or a font file.
Use it for titles, labels and paragraphs; animate the layer to move or fade the
prepared text. See [sources and layers](../../guides/python/sources-and-layers.md)
to add it to a project and export the result.

## API signatures

```text title="Source signature summary (presentation keywords omitted)"
Text(text: str, *, font: str | PathLike[str] | None = None,
     font_size=48, fill="#ffffff", align="left", max_width=None,
     line_spacing=1.0, letter_spacing=0.0)
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
| Non-empty `str` | Installed family or file, using the [path-disambiguation rules](#family-or-file). |
| `PathLike[str]` | Always a file path. |

The bundled font needs no host setup and provides Latin, Greek, Cyrillic and
other Unicode glyphs. An explicit font file is portable when the same licensed
file is available. Installed families depend on the machine where a snapshot
is created; the snapshot then references the resolved file. There is no automatic
per-character font fallback.

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
| Construction | Invalid text/font types, empty font selector, invalid color, alignment or numeric layout values. |
| Snapshot creation | An unavailable installed family raises `ValueError`; there is no fallback. |
| Preflight/preparation | A selected font file must exist and load successfully. |

## Canonical JSON

The source tag is `text`. Fields are `text`, `font`, `font_size`, `fill`, `align`,
optional `max_width`, `line_spacing` and `letter_spacing`. In a project snapshot,
`font` is a registered **Font asset ID**, rather than the Python selector.
See [project format](../project-format.md) for the surrounding asset and clip objects.
