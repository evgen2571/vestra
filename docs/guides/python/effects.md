# Add and animate an effect

Start with a [layer](sources-and-layers.md) in a project. An effect changes its
appearance; it does not alter its timeline placement. This example blurs a
procedural title, so no input files are needed.

```python
from vestra import Color, Project, Text
from vestra.effects import GaussianBlur

project = Project(
    size=(640, 360),
    fps=30,
    duration=2,
)

project.root.add(Color("#101b32"))
title = project.root.add(Text("A changing focus", font_size=40))

blur = title.effects.add(GaussianBlur(radius=0))
blur.radius.keyframe(0, 0)
blur.radius.keyframe(2, 6)
```

<span id="animate-an-effect"></span>

Keep the handle returned by `add()`: it refers to the attached copy. Reusing a
`GaussianBlur` value on two layers creates independent attachments. Editing the
original value afterwards does not edit either layer. The blur grows during
these two seconds using the same [keyframe rules](../../getting-started/animation.md)
as transforms. Layer effect keyframes use layer-local time, even when the layer
starts later in its composition.

## Choose where the effect applies

| Attachment | What it processes | Useful for |
| --- | --- | --- |
| `title.effects` | One layer's presentation | Blur a title without blurring the background. |
| A group/composition layer's `effects` | The composed child result | Apply one look to a card and everything inside it. |
| `project.post_effects` | Final root composition | Give all layers one consistent treatment. |

Continue the project above with a subtle global adjustment:

```python
from vestra.effects import Brightness

project.post_effects.add(Brightness(amount=0.05))
project.render("focus.mp4", backend="cpu", overwrite=True)
```

Run the two blocks in one Python file with `python focus.py`. It writes a
2-second `focus.mp4` in the current directory and replaces it on a rerun.
Global effect tracks use project time. A reusable `Group` snapshots its children;
attach processing to the returned placed layer when editing that instance.

## Predict the order

Effects in the same stage run in attachment order. Blur followed by quantization
makes crisp palette dots from softened content. Quantization followed by blur
softens those dots and introduces intermediate colors. Inspect the
[controlled order comparison](../../concepts/effects-transitions-and-animation.md#order-changes-the-result)
and [pipeline diagram](../../concepts/effects-transitions-and-animation.md#processing-stages)
before combining treatments.

Motion Tile is the exception to ordinary post-transform image processing: its
intrinsic stage is before transform. Inserting it later in a list does not move
it after a blur. It is layer-only and supports image, video, shape, text and
group content. Masks and track mattes consume the effected layer presentation.

<span id="fill-the-edges-of-a-moving-image"></span>
<span id="reduce-colours-with-dithering"></span>
<span id="try-halftone-sorting-or-crt"></span>
<span id="turn-image-detail-into-characters"></span>
<span id="adjust-palette-spacing-and-interpolation"></span>
<span id="adjust-detail-before-quantization"></span>
<span id="render-a-treatment"></span>

## Choose a treatment

Start with the [visual effect catalog](effect-catalog.md). Every entry shows
representative settings, the same baseline and a route to runnable source.
Then use a focused guide when a treatment needs more decisions:

- [Palettes and dithering](palettes-and-dithering.md): tonal versus color matching,
  pattern size, contrast and input detail.
- [ASCII](ascii.md): glyph cells, source colors, alpha and readability.
- [Composed treatments](#fill-moving-image-edges): edge filling, halftone and monitor recipes.

The [parameter reference](../../reference/effect-parameters.md) distinguishes
bindable properties from static enums/colors/integers. Bindable controls can use
[signals](signals-and-audio-reactivity.md). Validate after choosing settings;
exact ranges, ownership, scope and backend limits are in
[effect contracts](../../reference/effects.md) and [feature support](../../reference/feature-support.md).

## Fill moving image edges

Use Motion Tile before moving, rotating or scaling content when transparent edges
would otherwise reveal the background. This asset-free example deliberately
shrinks a card so the repeated region is visible:

```python
from vestra import Color, Group, Project, Rectangle, Text
from vestra.effects import DirectionalBlur, MotionTile

project = Project(
    size=(640, 360),
    fps=30,
    duration=2,
)

card = Group(size=(320, 180))
card.add(Color("#101b32"))
card.add(Rectangle(width=120, height=8, fill="#d6a46b"))
card.add(Text("Repeated card", font_size=24), position=(0.5, 0.7))

layer = project.root.add(card, anchor=(0, 0), position=(0.25, 0.25))

layer.effects.add(
    MotionTile(
        output_width_percent=400,
        output_height_percent=400,
        mirror_edges=True,
    )
)

layer.transform.rotation.keyframe(0, -12)
layer.transform.rotation.keyframe(2, 12)

layer.effects.add(DirectionalBlur(radius=4, angle_degrees=25))

project.render("tiled-card.mp4", backend="cpu", overwrite=True)
```

Run this complete program with `python tiled_card.py`; it replaces
`tiled-card.mp4` on a rerun. Notice mirrored repeats around the original card,
then blur in the rotated output. Motion Tile samples a virtual source region;
the 640×360 canvas stays the same size. The explicit origin anchor places the
card's top-left; expanding the region preserves that placement for any anchor.
Its stage is intrinsically before transform,
even if it is attached after an ordinary effect. Check
[source restrictions](../../reference/effects.md#motion-tile) before attaching it.

## Apply a monitor recipe

For this alternative, start a fresh project rather than accumulating the previous
example's tiling and blur:

```python
from vestra import Color, Project, Text
from vestra.effects.recipes import analog_monitor

project = Project(
    size=(640, 360),
    fps=30,
    duration=2,
)

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
