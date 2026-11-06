---
description: Choose reusable groups or editable child compositions and understand their transforms, clipping and local time.
---

# Groups and nesting

A group composes several children into one visual result, then places that result
in its parent. This lets a title, its background and its decoration move together
while retaining their own relative positions and timing.

## Choose how to author the children

| Need | Python interface | What later edits affect |
| --- | --- | --- |
| Reuse a card or badge | Create a `Group`, then `composition.add(group)` | Each placement copies the children and state present when added |
| Keep editing one scene | Call `composition.group()`, then edit its `child` | The owned child composition remains part of that placement |

A reusable group is a source, not a live template. Finish adding its badge before
placing the card. Later edits to the original group or its original child handles
do not update earlier placements. A direct `CompositionLayer` exposes `add()` and
`child.add()` for continued editing, including transitions between its children.
See [the nested composition guide](../guides/python/nested-compositions.md) for
both workflows.

## Compose locally, then transform the result

<figure markdown="span">
![A 600 by 300 local group canvas contains a rectangle centered at 150, 150. Another rectangle crosses its upper edge and is clipped. The group is then scaled by one half and placed at the center of a 640 by 360 parent canvas.](../assets/diagrams/groups-local-canvas.svg)
<figcaption>Schematic sequence. The dashed area above the local canvas is clipped away before the parent transform moves and scales the group.</figcaption>
</figure>

`Group(size=(600, 300))` gives its children a local canvas. A child at
`position=(0.25, 0.5)` has its anchor at `(150, 150)` there. A 100 × 80 rectangle
remains 100 × 80 local pixels. With a group scale of `0.5`, it occupies 50 × 40
pixels in the parent before any other ancestor transforms.

An unsized group inherits its immediately enclosing canvas. That also applies to
the child composition created by `composition.group()`, which has no `size`
argument. Use a sized `Group` when reusable content needs dimensions independent
of the project resolution. A nested sized group starts another local coordinate
space; an unsized child inherits its parent's space.

## Clipping happens before the parent transform

Children are clipped to their group's local canvas. Content outside it is lost
before the group is transformed in its parent. Moving or shrinking the group
cannot bring that clipped content back. Move the child inside the local canvas
or enlarge the group when you need to preserve it. There is no overflow-visible
option.

Child effects run in the local canvas. Effects on the group run on its completed,
transformed placement in the parent, so an outer blur can spread beyond its
bounds. A group mask applies to the already-composited group and follows its
placement. [Masks and mattes](masks-and-mattes.md) explain this distinction.

## Child time starts with the group

If a group starts at parent second `1` and a child starts at local second `0.5`,
the child starts at parent second `1.5`. The group's lifetime clips the children;
a child extending past it does not keep the group alive. Changing parent start
moves the whole scene without rewriting child starts or keyframes.

Transitions between children belong to `group.child.transitions`. A transition
using the group as a parent-level endpoint belongs to the parent composition.
The [timeline concept](timeline-and-time.md) shows the clocks side by side.

For fractional sizes, resource limits and exact transform rules, see
[nested composition reference](../reference/nested-compositions.md#sized-group-coordinates-and-bounds).
