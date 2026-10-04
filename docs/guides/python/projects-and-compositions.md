# Projects and compositions

A project defines the video size, frame rate and duration. Its root composition
holds the visual layers that appear in the video.

```python
import vestra
from vestra.sources import Color, Rectangle


project = vestra.Project(size=(1280, 720), fps=30, duration=5)
project.root.add(Color("#111827"), duration=5, id="background")

# Show a blue panel from second 1 until second 3.
panel = project.root.add(
    Rectangle(width=600, height=180, fill="#2563eb"),
    start=1,
    duration=2,
    z=1,
    id="panel",
)
```

`start` and `duration` control when a layer appears. `z` controls which layers
appear on top: higher values draw over lower ones. IDs are optional, but naming
important layers makes diagnostic messages and later edits easier to follow.

## Keep related layers in a group

Use `group()` to keep several layers together. Moving, scaling or rotating the
group also transforms its children:

```python
card = project.root.group(start=1, duration=3, z=2, id="card")

card.child.add(
    Rectangle(width=500, height=220, fill="#f9fafb"),
    duration=3,
    id="card-background",
)

# A coloured stripe appears a little after the card itself.
card.child.add(
    Rectangle(width=360, height=12, fill="#2563eb"),
    start=0.25,
    duration=2,
    z=1,
    id="card-stripe",
)

card.transform.rotation_degrees.keyframe(0, -5)
card.transform.rotation_degrees.keyframe(3, 5)
```

Children use time measured from the beginning of their group. The card starts
at second 1 of the project, so its stripe appears at second 1.25. The group's
rotation runs over its own three-second duration and affects both rectangles.
See [nested compositions](nested-compositions.md) for more examples.

## Set a duration

An explicit project duration makes the output length predictable. Without one,
non-video layers need either an explicit layer duration or a duration supplied
by their composition.

Continue with [sources and layers](sources-and-layers.md) to add media or text,
or [rendering and preparation](rendering-and-preparation.md) to export the
project and understand validation and runtime checks.
