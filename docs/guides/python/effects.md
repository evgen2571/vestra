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
these two seconds using the same [keyframe rules](../../learn/animation.md)
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
- [Composed treatments](effect-treatments.md): edge filling, halftone and monitor recipes.

The [parameter reference](../../reference/effect-parameters.md) distinguishes
bindable properties from static enums/colors/integers. Bindable controls can use
[signals](signals-and-audio-reactivity.md). Validate after choosing settings;
exact ranges, ownership, scope and backend limits are in
[effect contracts](../../reference/effects.md) and [feature support](../../reference/feature-support.md).
