# Sources and layers

A source describes visual content. Adding it to a composition creates a layer
that supplies timing and presentation.

```python
from vestra.sources import Color, Image, Rectangle, Text, Video

project.root.add(Image("assets/photo.png"), start=0, duration=5, id="photo")
project.root.add(Video("assets/clip.mp4"), start=1, duration=3, id="clip")
project.root.add(
    Rectangle(width=600, height=180, fill="#0f172a"),
    start=2,
    duration=2,
    z=2,
    id="panel",
)
```

Other current source families include `Color`/`SolidColor`, `Text`,
`Spectrum2D`, and `ParticleSystem`. Shapes are represented by concrete
classes such as `Rectangle`, `Ellipse`, `Circle`, `Line`, and `Polygon`.
Image and video sources can carry sizing and crop properties.

Text can use the bundled default, an installed font family, or an explicit file:

```python
from vestra import Text, list_system_fonts

title = Text("Hello", font_size=48)  # bundled default; font=None does the same
print(list_system_fonts())          # case-insensitively sorted, unique families
title = Text("Hello", font="DejaVu Sans", font_size=48)  # if installed
title = Text("Hello", font="assets/MyFont.ttf", font_size=48)
```

The bundled DejaVu-derived regular font works without host fonts and provides
Latin, Greek, Cyrillic and other Unicode glyphs. It produces consistent font
selection across machines. Explicit files are portable when the same file is
available. Installed families depend on the machine where the snapshot is
created; the snapshot then references the resolved file.

Family matching is exact and case-insensitive, including names with spaces.
Vestra prefers a regular face and breaks ties by file path and collection index.
An unavailable family raises `ValueError` during snapshot creation; it never
silently substitutes another family. Use `list_system_fonts()` to see choices.

`PathLike` values always mean files. Strings containing `/`, `\\`, or a filename
suffix (such as `Manrope.ttf`) mean files too, as do existing files relative to
`base_directory`. Other strings mean family names. For an extensionless file
that does not yet exist, use `Path("myfont")` or `"./myfont"`. For a family name
containing a dot or slash, discover its file and provide that explicit path.
Relative file paths retain their project base-directory semantics. Missing,
inaccessible or invalid files fail preflight/preparation.

Discovery is cached for the process: restart Python after installing fonts or
changing font configuration. Linux uses fontconfig configuration (including
`FONTCONFIG_FILE` and configured NixOS directories) with standard-directory
fallback. Windows and macOS scan standard system/user font directories;
application-private fonts, macOS resource-fork/dfont files, Windows registry-only
custom locations and uninstalled cloud fonts may not appear. Fontconfig aliases
and substitution rules do not replace exact family matching. No external font
utilities are needed. Empty discovery returns `[]`. Collections use the chosen face through a standalone
file in the user cache (`vestra/fonts` under the platform cache directory;
`XDG_CACHE_HOME` on Linux). Keep that file available for saved snapshots, or
restart Python and recreate the snapshot after clearing caches. The default is bundled in the
package and does not need a writable cache. There is no automatic per-character
font fallback. See the [Text reference](../../reference/sources/text.md).

`Color`/`SolidColor` fills the canvas in canonical JSON, so it has no direct
canonical transform or transition endpoint. In Python, its layer can still be
transformed or used in a transition. The authoring API supplies the needed
presentation wrapper when it lowers the project.

Configure the returned layer for the edit:

```python
layer = project.root.add(Image("assets/photo.png"), duration=5)
layer.opacity.value = 0.85
layer.transform.scale.value = 1.1
layer.transform.position.value = (0.5, 0.45)
```

Use `CompositionLayer` through `composition.group()` for nested content. A
child composition is not an ordinary source in the high-level taxonomy.
