---
description: Set output dimensions and timing, add ordered layers and move related content as one child composition.
---

# Projects and compositions

A project defines the video size, frame rate and duration. Its root composition
holds the visual layers that appear in the video. Use this guide for setup;
[the authoring model](../../concepts/authoring-model.md) explains sources,
placements and snapshot copies.

```python
from vestra import Group, Project
from vestra.sources import Color, Rectangle

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
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

Build a standalone `Group`, add its children, then place the completed source.
Moving, scaling or rotating the returned placement also transforms its children:

```python
card = Group(size=(500, 220))

card.add(
    Rectangle(width=500, height=220, fill="#f9fafb"),
    duration=3,
)

# A coloured stripe appears a little after the card itself.
card.add(
    Rectangle(width=360, height=12, fill="#2563eb"),
    start=0.25,
    duration=2,
    position=(0.5, 0.7),
)

card_layer = project.root.add(card, start=1, duration=3, z=2, id="card")
card_layer.transform.rotation.keyframe(0, -5)
card_layer.transform.rotation.keyframe(3, 5)
```

Children use time measured from the beginning of their group. The card starts
at second 1 of the project, so its stripe appears at second 1.25. The group's
rotation runs over its own three-second duration and affects both rectangles.
Finish building `card` before placing it: `add(card)` copies its current contents.
The [groups concept](../../concepts/groups-and-nesting.md) shows local coordinates,
local time and the difference from `root.group()`. Use
[nested compositions](nested-compositions.md) for reuse and direct child editing.

## Set a duration

An explicit project duration fixes the output length. Content extending beyond
it is clipped, and shorter content can leave a background or silent tail.
Without one, non-video layers need an explicit duration or a duration supplied
by their composition; video layers can use their remaining media duration.

An omitted non-video duration inherits the whole containing duration, even when
`start` is later than zero. It does not mean “until the project ends.” For a layer
starting at `1` in a five-second project, use `duration=4` to end at `5`.
[Timeline and time](../../concepts/timeline-and-time.md#duration-is-not-an-end-timestamp)
explains automatic duration and frame boundaries.

Continue with [sources and layers](sources-and-layers.md) to add media or text,
or [rendering and preparation](rendering-and-preparation.md) to export the
project and understand validation and runtime checks.
