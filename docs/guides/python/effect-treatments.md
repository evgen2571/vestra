# Compose an effect treatment

Start with [one effect](effects.md) and use the [catalog](effect-catalog.md) to
find settings that reveal the intended change. A recipe is an ordered list of
ordinary effects, so its order and individual controls still matter.

## Fill moving image edges

Use Motion Tile before moving, rotating or scaling content when transparent edges
would otherwise reveal the background. This asset-free example deliberately
shrinks a card so the repeated region is visible:

```python
from vestra import Color, Group, Project, Rectangle, Text
from vestra.effects import DirectionalBlur, MotionTile

project = Project(size=(640, 360), fps=30, duration=2)
card = Group(size=(320, 180))
card.add(Color("#101b32"))
card.add(Rectangle(width=120, height=8, fill="#d6a46b"))
card.add(Text("Repeated card", font_size=24), position=(0.5, 0.7))
layer = project.root.add(card, anchor=(0, 0), position=(0.25, 0.25))
layer.effects.add(MotionTile(
    output_width_percent=400,
    output_height_percent=400,
    mirror_edges=True,
))
layer.transform.rotation.keyframe(0, -12)
layer.transform.rotation.keyframe(2, 12)
layer.effects.add(DirectionalBlur(radius=4, angle_degrees=25))
project.render("tiled-card.mp4", backend="cpu", overwrite=True)
```

Run this complete program with `python tiled_card.py`; it replaces
`tiled-card.mp4` on a rerun. Notice mirrored repeats around the original card,
then blur in the rotated output. Motion Tile samples a virtual source region;
the 640×360 canvas stays the same size. The explicit origin anchor avoids the current
[center-anchor limitation](../../development/documentation.md#motion-tile-anchor-limitation).
Its stage is intrinsically before transform,
even if it is attached after an ordinary effect. Check
[source restrictions](../../reference/effects.md#motion-tile) before attaching it.

## Apply a monitor recipe

For this alternative, start a fresh project rather than accumulating the previous
example's tiling and blur:

```python
from vestra import Color, Project, Text
from vestra.effects.recipes import analog_monitor

project = Project(size=(640, 360), fps=30, duration=2)
project.root.add(Color("#101b32"))
project.root.add(Text("Signal monitor", font_size=42))
for effect in analog_monitor(period=2):
    project.post_effects.add(effect)
project.render("monitor.mp4", backend="cpu", overwrite=True)
```

This writes `monitor.mp4`. Recipe attachments are independent copies, so keep
returned handles if changing them afterwards. `halftone_print()` and
`sorted_neon(direction=...)` provide other composed starting points. They do not
change which layer types a cinematic preset accepts. Periodic CRT motion repeats
the procedural component; it does not loop your footage or soundtrack.

## Decide what to animate

Fade a look with `amount` when available. Halftone's cell geometry can change
membership as it moves; PixelSort's threshold crossings deliberately create sharp
run changes. These effects do not promise a smooth crossfade between static
modes. Inspect motion at the output size, especially after compression.

See the [processing model](../../concepts/effects-transitions-and-animation.md),
[exact parameters](../../reference/effect-parameters.md), and
[backend support](../../reference/feature-support.md). Broader moving treatments
remain available in the [rendered gallery](../../gallery/visual-effects.md).
