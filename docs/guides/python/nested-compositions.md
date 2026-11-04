# Use nested compositions

A standalone `Group` is reusable visual content. Each `add()` snapshots the
source, so placing the same group again creates independent children and
presentation state:

```python
from vestra.sources import Group, Rectangle, Circle

card = Group()
card.add(Rectangle(width=200, height=100, fill="#ff0000",
                   position=(0.4, 0.5), rotation=15))
card.add(Circle(radius=30, fill="#2563eb", position=(0.6, 0.5)))
project.root.add(card, duration=5)
second = project.root.add(card, duration=5, position=(0.75, 0.5))
second.transform.rotation.keyframe(0, 0)
second.transform.rotation.keyframe(5, 180)
```

`Group.add()` returns a child placement with `transform`, `opacity`, `effects`,
`masks`, and `blend_mode`. Child `start` is group-local seconds; omitted child
`duration` inherits the containing placement's full lifetime, including for
video children. Set `source_start` and `playback_rate` on video children to trim
and retime them. `layer` controls child ordering and defaults to insertion order.
Groups nest by adding another `Group`. Source [transform defaults](sources-and-layers.md)
and individual placement overrides work on both `Group.add()` and
`Composition.add()`. Coordinates remain normalized to the parent canvas;
there is no group size or separate group-local coordinate system.

For direct authoring, `composition.group()` creates one `CompositionLayer`
that owns an editable child composition. Add children through `group.add()`
or its `child` composition:

```python
group = project.root.group(start=1, duration=3, z=2, id="scene")
group.add(Color("#0f172a"), duration=3)
# This starts 0.5 seconds into the group, at project time 1.5 seconds.
badge = group.child.add(Color("#f97316"), start=0.5, duration=1.5, z=1)
badge.transform.scale.value = 0.5
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
