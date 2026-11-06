---
description: Reuse a group snapshot or edit one child composition, with explicit local dimensions and timing.
---

# Use nested compositions

Choose a reusable `Group` when placing copies of the same artwork. Use
`composition.group()` when you want to keep editing one owned child composition.
Both compose children locally before placing their result in the parent. The
[groups concept](../../concepts/groups-and-nesting.md) explains transforms,
clipping and ownership visually.

## Snapshot a reusable group

A standalone `Group` is reusable visual content. Each `add()` snapshots the
source, so placing the same group again creates independent children and
presentation state:

```python
from vestra import Circle, Color, Group, Project, Rectangle

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
card = Group()
card.add(
    Rectangle(width=200, height=100, fill="#ff0000", position=(0.4, 0.5), rotation=15)
)
card.add(Circle(radius=30, fill="#2563eb", position=(0.6, 0.5)))
project.root.add(card, duration=5)
right_card = project.root.add(card, duration=5, position=(0.75, 0.5))

right_card.transform.rotation.keyframe(0, 0)
right_card.transform.rotation.keyframe(5, 180)
```

`Group.add()` returns a child placement with `transform`, `opacity`, `effects`,
`masks`, and `blend_mode`. Child `start` is group-local seconds; omitted child
`duration` inherits the containing placement's full lifetime, including for
video children. Set `source_start` and `playback_rate` on video children to trim
and retime them. `layer` controls child ordering and defaults to insertion order.
Groups nest by adding another `Group`. Source [transform defaults](sources-and-layers.md)
and individual placement overrides work on both `Group.add()` and
`Composition.add()`.

## Give reusable content local dimensions

Set `size=(width, height)` to position children independently of the output
resolution. Dimensions are positive, finite pixel values; odd and
fractional values are supported. The group's placement uses its parent's coordinates:

```python
card = Group(size=(600, 300))
card.add(Rectangle(width=100, height=80, fill="#2563eb", position=(0.25, 0.5)))
```

The rectangle's anchor is at `(150, 150)` inside the card. The card's own anchor
is placed at the center of its parent. The rectangle remains 100×80 pixels
before placement transforms; `size` does not stretch its geometry.

Sized groups nest with their own dimensions. Add the badge before placing the
card: placement copies the children that exist at that moment.

```python
badge = Group(size=(120, 60))
badge.add(Circle(radius=12, fill="#ffffff", position=(0.5, 0.5)))
card.add(badge, position=(0.75, 0.5))
project.root.add(card, duration=5, position=(0.5, 0.5))
```

Here the badge is placed at `(450, 150)` in the card and its circle is at
`(60, 30)` in the badge. Unsized groups inherit the containing coordinate
system. Existing unsized content requires no migration. Sizes survive source
snapshots, reuse in projects with different resolutions, and owned mask
serialization. Child-local timing and the project's FPS remain unchanged.

Children clip to the local canvas **before** parent transforms. Scaling down a
group cannot recover children already outside its local bounds. Size sets the
canvas; it does not scale the rectangle or font geometry. See the
[coordinate diagram](../../concepts/groups-and-nesting.md#compose-locally-then-transform-the-result)
and [exact bounds rules](../../reference/nested-compositions.md#sized-group-coordinates-and-bounds).

## Edit a child composition directly

For direct authoring, `composition.group()` creates one `CompositionLayer`
that owns an editable child composition. Add children through `group.add()`
or its `child` composition:

```python
project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
group = project.root.group(start=1, duration=3, z=2, id="scene")
group.add(Color("#0f172a"), duration=3)
badge = group.child.add(Color("#f97316"), start=0.5, duration=1.5, z=1)
badge.transform.scale = 0.5
```

The group starts at parent time one. `badge` starts at child time 0.5, so it
appears at parent time 1.5. Transforms, opacity, effects, and transitions on
the `CompositionLayer` affect the child placement in the parent. The child
layers retain their own local properties.

Use `group.child.transitions` for transitions between child siblings. Use
`project.root.transitions` when the child composition itself is one endpoint
of a parent transition.

Use the standalone style for reusable templates; edits to its source after
placement do not change existing layers. Use the direct style to keep editing
one owned child composition, including transitions between its siblings.
Standalone group placements can also be transition endpoints in their parent.
Both styles lower to the same canonical group clips and share media/font assets.
Groups remain supported as owned mask inputs.

Nesting is recursive, subject to the project's validation limits. Keep a
composition's timing explicit and validate after building deep or dynamically
generated graphs.

## Resource limits and exact bounds

Default resource limits allow group canvases up to 8192 pixels per axis and
67,108,864 pixels. Larger canvases require an explicit execution budget:

```python
from vestra import Project, ResourceLimits

limits = ResourceLimits(maximum_group_width=16_384, maximum_group_pixels=640 * 360)
project = Project(
    size=(640, 360),
    fps=30,
    duration=2,
    resource_limits=limits,
)
strip = Group(size=(16_384, 2))
strip.add(Rectangle(width=16_384, height=2, fill="#2563eb"))
project.root.add(strip, scale=0.025)
```

The pixel budget must also cover unsized groups, which inherit the project canvas.
Here the 640 × 360 project needs more pixels than the wide, two-pixel strip.

The same limits apply to validation, preparation and rendering, including owned
group masks. Budgets do not change coordinates, clipping or backend limits.

Fractional dimensions use raster bounds rounded up to whole pixels while
coordinates and anchors retain their authored units. Child effects use the
local canvas; group effects use its completed placement in the parent. Group
masks follow their owner, and sized group mask sources retain their own dimensions.
See [exact coordinate and effect rules](../../reference/nested-compositions.md#sized-group-coordinates-and-bounds).

The advanced builder accepts the same `size` keyword on
`ProjectBuilder.add_group_clip(...)`. Both paths serialize an optional
`"size": {"width": 600.0, "height": 300.0}` on the canonical group source.
