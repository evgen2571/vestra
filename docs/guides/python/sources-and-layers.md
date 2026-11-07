---
description: Place independent copies of shapes, media and text, then edit the returned layers.
---

# Sources and layers

Add a source to a composition, then configure the returned layer. A source
supplies content; the layer supplies timing, placement and processing. If these
roles are new, start with [the authoring model](../../concepts/authoring-model.md).

## Reuse content in independent placements

This complete setup needs no media files:

```python
from vestra import Project, Rectangle

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)

panel = Rectangle(width=200, height=100, fill="#2563eb")

left_panel = project.root.add(panel, position=(0.25, 0.5))
right_panel = project.root.add(panel, position=(0.75, 0.5))
right_panel.opacity = 0.85
right_panel.transform.rotation = 15
```

Each `add()` copies the source and its default transform. The two placements
can have different transforms, opacity, effects, masks and timing. Later edits
to `panel` affect future placements; edit `right_panel` to change the existing
right rectangle. For file-backed media, copying the source retains the file
reference rather than copying the file on disk.

Source geometry uses pixels, while placement uses normalized parent coordinates.
The rectangle remains 200 × 100 pixels before transforms; `(0.75, 0.5)` places
its default center anchor three quarters across the canvas. See
[coordinates and anchors](../../concepts/coordinates-and-anchors.md).

## Add media to that project

Use your own `assets/photo.png` and `assets/clip.mp4` with at least three seconds
of footage. Paths are relative to the project base directory, which defaults to
the directory where you create the project.

```python
from vestra.sources import Image, Video

photo = Image("assets/photo.png")
footage = Video("assets/clip.mp4")

portrait = project.root.add(photo, duration=5, id="photo")
project.root.add(footage, start=1, duration=3, z=1, id="clip")
portrait.transform.scale = 1.1
portrait.transform.position = (0.5, 0.45)
```

Images and videos can use sizing and crop options; their exact behavior is in the
[image](../../reference/sources/image.md) and [video](../../reference/sources/video.md)
references. A source offset and playback rate select a different part of a video;
see [timeline and time](../../concepts/timeline-and-time.md).

## Choose another source

`Color` (also `SolidColor`) fills the canvas. Shape classes include `Rectangle`,
`Ellipse`, `Circle`, `Line` and `Polygon`. `Text` uses font layout. `Spectrum2D`
and `ParticleSystem` generate content over time. Reusable `Group` sources combine
children; see [nested compositions](nested-compositions.md).

Procedural shapes can use default geometry and style:

```python
square = Rectangle()
outline = Rectangle(stroke="#00ff00")

project.root.add(square)
project.root.add(outline, position=(0.7, 0.5))
```

The square is white and 100 × 100 pixels. Supplying only a stroke creates a
two-pixel outline. `Color()` defaults to opaque black. Image/video paths, text
content and polygon points remain required. See [shape defaults](../../reference/sources/shape.md).

## Add text and choose a portable font

Text can use the bundled default, an installed font family, or an explicit file:

```python
from vestra import Text, list_system_fonts

title = Text("Hello", font_size=48)
project.root.add(title)
```

The bundled font needs no host font setup. Use an explicit font file when the
project requires another face and distribute it under its license.

??? note "Installed families, file paths and font discovery"

    To choose an installed family, check which names are available first:

    ```python
    print(list_system_fonts())

    system_title = Text("Hello", font="DejaVu Sans", font_size=48)
    ```

    Use that family only if it appears in the list. For a font file you distribute
    with the project, provide its path instead:

    ```python
    file_title = Text("Hello", font="assets/MyFont.ttf", font_size=48)
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
    utilities are needed. Empty discovery returns `[]`. Collections use the chosen
    face through a standalone file in the user cache (`vestra/fonts` under the platform cache directory;
    `XDG_CACHE_HOME` on Linux). Keep that file available for saved snapshots, or
    restart Python and recreate the snapshot after clearing caches. The default
    is bundled in the package and does not need a writable cache. There is no automatic per-character
    font fallback. See the [Text reference](../../reference/sources/text.md).

## Set source transform defaults

Visual sources accept `position`, `anchor`, `scale`, and `rotation` (alias
`rotation_degrees`) as constructor defaults. For example:

```python
panel = Rectangle(
    width=200, height=100, fill="#ff0000", position=(0.4, 0.5), rotation=15
)
left_panel = project.root.add(panel)
right_panel = project.root.add(panel, position=(0.75, 0.5))

right_panel.transform.rotation.keyframe(0, 15)
right_panel.transform.rotation.keyframe(2, 180)
```

These values initialize the placement's existing transform; they add no extra
rendering pass. Position is normalized to the parent canvas, anchor is within
`[0, 1]`, scale is a positive number or pair, and rotation is finite degrees.
Defaults remain position/anchor `(0.5, 0.5)`, scale `(1, 1)`, rotation `0`.
ParticleSystem's existing `rotation` controls particle orientation; use
`rotation_degrees` for its presentation default.

You can configure `source.transform` before placement, including keyframes and
component audio bindings, or pass `transform=my_transform` to a constructor or
`add()`. Every placement copies all transform state. A supplied `transform`
replaces the entire default transform and cannot be mixed with individual
transform arguments. Supplying both rotation spellings is also an error.

Individual `add()` overrides replace the selected property's value, keyframes,
and bindings, including its X/Y component bindings. Unspecified properties
retain their complete state. Later edits through `layer.transform` use the
normal property behavior; assigning a numeric `.value` preserves its existing
keyframes and bindings. Later source edits affect only future placements.

Scale bindings apply in order: uniform `transform.scale` bindings, then
`transform.scale.x`/`.y`, then `transform.scale_x`/`scale_y` bindings.

`Color`/`SolidColor` fills the canonical canvas and has no direct canonical
transform or transition endpoint. Python supplies a presentation wrapper when
its layer is transformed or used in a transition.

For animation timing and interpolation, continue with
[animation and keyframes](../../learn/animation.md). For reusable multi-part
content, continue with [nested compositions](nested-compositions.md).
