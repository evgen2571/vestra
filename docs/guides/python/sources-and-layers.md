---
description: Place independent copies of shapes, media and text, then edit the returned layers.
---

<span id="sources-and-layers"></span>

# Add content and edit layers

Add a source to a composition, then configure the returned layer. A source
supplies content; the layer supplies timing, placement and processing. If these
roles are new, start with [the authoring model](../../concepts/authoring-model.md).

## Reuse content in independent placements

Start a new `sources.py` with this setup; it needs no media files. The later
blocks continue this project unless stated otherwise:

The render calls below use `overwrite=True`; rerunning a script replaces its
named output files.

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
and [`ParticleSystem`](../../reference/sources/particle-system.md) generate
content over time. Reusable `Group` sources combine children; see
[nested compositions](nested-compositions.md).

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
from vestra import Text

title = Text("Hello", font_size=48)
project.root.add(title)
```

The bundled font needs no host font setup. For another face, supply a licensed
font file as `assets/MyFont.ttf` before adding this block:

```python
file_title = Text("Hello", font="assets/MyFont.ttf", font_size=48)
project.root.add(file_title, position=(0.5, 0.7))
```

Leave `font` unset for consistent selection without host font setup. Installed
families, path selection, discovery and cache behavior are covered by the
[Text reference](../../reference/sources/text.md#select-a-font).

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

These values initialize each layer's transform. Here both rectangles inherit
15° rotation, while only the right one overrides position. Keyframes then edit
that returned layer without changing the source or the left rectangle.

Use `source.transform` to prepare more defaults before placement. For exact
limits, `transform=` replacement and animation/binding precedence, see
[source presentation](../../reference/python-api.md#source-presentation).

## Export the result

After the edits you want to try, finish `sources.py` with:

```python
project.render("sources-and-layers.mp4", backend="cpu", overwrite=True)
```

```bash
uv run python sources.py
```

This writes `sources-and-layers.mp4` in the current directory and replaces it
on a rerun. The shape and default-font examples need no inputs; include the
media and custom-font blocks only after supplying their listed files.

For animation timing and interpolation, continue with
[animation and keyframes](../../getting-started/animation.md). For reusable multi-part
content, continue with [nested compositions](nested-compositions.md).
