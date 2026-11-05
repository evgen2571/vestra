# Nested compositions

High-level Python supports reusable `Group` sources and owned `CompositionLayer`
placements. Both lower to the canonical visual source with type `group`.

`Composition.group(name=None, *, start=0, duration=None, z=0, visible=True,
opacity=1, id=None, blend_mode=BlendMode.NORMAL)` creates a parent placement
with a child composition. Child layer times are local to the child.
The group start and duration are local to the parent. Child transforms are
evaluated in the child, then the parent placement applies its placement and
transform.

Transitions connect layers owned by one composition. A child composition can
therefore contain its own transitions. Current tests cover nested transition
compilation. Endpoints must be owned by the same
composition and satisfy the transition's timing and fit constraints.

Groups serialize inside the canonical visual tree. Recursive nesting is
supported subject to validation limits and timing constraints. CPU and WGPU
support nested composition evaluation; adapter selection and media readiness
remain backend and environment concerns.

## Sized group coordinates and bounds

`Group(size=(600, 300))` establishes a 600 by 300 pixel local canvas.
Canonical JSON represents this as `"size": {"width": 600, "height": 300}`.
Both dimensions must be positive finite numbers whose rounded-up raster extents
fit `u32`; the RGBA canvas byte length must also be addressable. Odd and fractional
dimensions are allowed. The size is static and belongs to the source. Default
resource budgets permit up to 8192 pixels per axis and 67,108,864 pixels per
canvas; [configured limits](project-format.md#validation-layers) can permit larger
canvases when the selected backend supports them.

Child positions are normalized against that canvas: `(0.25, 0.5)` means
`(150, 150)` pixels. The group's own placement position uses its parent's canvas.
An unsized group inherits its immediately enclosing canvas, including a sized
parent. A nested sized group establishes its own dimensions again. Reusing a
source in a different project changes its placement coordinate system, while
its explicit local dimensions and child geometry stay the same.

For a child source point `q`, the placement maps it to
`position * parent_size + rotation(scale * (q - anchor * source_size))`.
Anchors use the unscaled source extent; for a group this is its local canvas.
Scaling precedes clockwise rotation, followed by translation. Parent placements
then apply to the completed child composition. Size alone does not scale a
rectangle, font, image, or effect radius. Animated positions, anchors, scale and
rotation use the same rules at each evaluated time; normalized positions may
extend outside `[0, 1]`.

Groups isolate and clip their content before parent presentation. A sized group
clips at its own local canvas; an unsized group retains the inherited-canvas
clipping behavior. There is no overflow-visible or optional clipping flag.
Off-bounds children can move into the local canvas through their own transforms,
but a later parent transform cannot restore content already clipped from the
group. A group larger than the output is composed at its local dimensions first,
so scaling it down preserves content throughout that local canvas.

Fractional dimensions retain their authored coordinate/anchor units and use a
raster canvas rounded up to whole pixels. The rasterized edge follows that
rounded extent; size does not request subpixel edge coverage. Group isolation,
sampling and antialiasing otherwise follow ordinary raster sources.

Solid colors and procedural sources fill/use the local canvas. Image `fit` and
`cover` sizing use that canvas; `original` retains intrinsic geometry. Child
effects operate on child layers in the local canvas. Effects attached to the
group operate on its completed, transformed layer in the parent canvas and may
extend beyond its bounds, such as a blur. Effect ordering is unchanged.

Group masks use owner-local coordinates and then follow the owning placement.
A sized group used as an owned mask source keeps its own local canvas and anchor;
an unsized mask group inherits the owner's coordinate canvas. Child masks,
ordered mask operations and sibling track mattes retain their existing rules.

Omitting `size` needs no migration: existing Python calls and canonical JSON
retain their dimensions, appearance, local timing, duration and project FPS.
See the [Python examples](../guides/python/nested-compositions.md).

Owned mask-source validation now reports the actual `masks/.../input/source`
JSON pointer, including invalid group sizes, rather than a synthetic clip-source
pointer. Diagnostic codes and messages are unchanged.
