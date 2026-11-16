---
description: Add a brief project-wide color overlay or a slow drift on one image layer.
---

# Add flashes and photo motion

Use a flash for a brief color overlay across the frame. Use an image preset for
a reusable motion treatment on one photograph. Neither needs the overlapping
endpoints used to [connect two shots](transitions-flashes-and-presets.md).

## Add a project-wide flash

Save this new script as `flash.py`; it needs no input files:

The render calls below use `overwrite=True`; rerunning a script replaces its
named output files.

```python
from vestra import Flash, Project, Text

project = Project(
    size=(640, 360),
    fps=30,
    duration=3,
    background="#101c24",
)

project.root.add(Text("A brief color overlay", font_size=32))

project.flashes.add(
    Flash(
        start=1.5,
        duration=0.12,
        colour="#ffffff",
        opacity=0.7,
        fade_out=0.08,
        layer=10,
    )
)

project.render("flash.mp4", backend="cpu", overwrite=True)
```

```bash
uv run python flash.py
```

This writes `flash.mp4` in the current directory and replaces it on a rerun.
The explicit `layer=10` places the overlay above the title. Use flashes sparingly:
their coverage is frame-wide, and draw order still applies. Placement uses
project seconds; a flash does not need a scene overlap.

## Give a photo a slow drift

Start a separate `slow_drift.py` with your own `assets/one.png`. Keep that image
path relative to the directory where you run the script:

```python
from vestra import Image, Project

project = Project(
    size=(640, 360),
    fps=30,
    duration=6,
)

photo = project.root.add(Image("assets/one.png"))
photo.presets.apply_slow_drift(start=0.5, duration=4, intensity=0.6)

project.render("slow-drift.mp4", backend="cpu", overwrite=True)
```

```bash
uv run python slow_drift.py
```

This writes `slow-drift.mp4` in the current directory and replaces it on a rerun.
The preset belongs to `photo`; the high-level API accepts one preset per Image
layer. It describes a reusable motion treatment rather than a change between
two layers. See [presets and flashes](../../reference/presets-and-flashes.md)
for timing, source restrictions and other constraints.
