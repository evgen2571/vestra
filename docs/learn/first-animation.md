---
description: Animate the geometric composition with position, opacity, scale and rotation keyframes.
---

# Your first animation

Animate the composition from [your first render](first-render.md). The circle
grows, the stripe rotates, and the text moves upward while fading in. The
caption fades in after a short delay. After 1.1 seconds, all four layers stay
at their final values until the video ends.

<figure>
  <video controls playsinline preload="metadata" poster="../../assets/renders/first-animation.webp" width="960" height="540" aria-label="Animated Shape study composition: a circle grows, a white stripe rotates, the heading moves up and fades in, then the caption appears.">
    <source src="../assets/renders/first-animation.mp4" type="video/mp4">
    <a href="../assets/renders/first-animation.mp4">Download the animation</a>.
  </video>
  <figcaption>Expected output: a three-second animation with no audio. Select play to see how the layers change.</figcaption>
</figure>

The steps below add keyframes to the existing composition. The
[complete script](#the-complete-script) includes all of the changes. Edit the
returned **layers**, because Vestra copies a source when you add it to a
composition.

## 1. Assign the circle layer to a variable

Add `Interpolation` to your imports. Then give the circle layer a name by
assigning the return value of `add`:

```python
from vestra import Ellipse, Interpolation, Project, Rectangle, Text

# Keep the Project(...) from the first render.
circle = project.root.add(
    Ellipse(width=210, height=210, fill="#f07860"),
    position=(0.78, 0.47),
)
```

`circle` is the placed layer. Its transform describes where the content is,
how large it is and how it rotates.

## 2. Animate scale and rotation

Immediately after creating it, add two keyframes:

```python
circle.transform.scale.keyframe(0, 0.4)
circle.transform.scale.keyframe(0.8, 1, interpolation=Interpolation.EASE_OUT)
```

At zero seconds the circle is 40% of its original size. At 0.8 seconds it reaches
100%. Vestra calculates the values between them. `EASE_OUT` starts quickly
and slows toward the final size.

**Put the easing on the destination keyframe.** Here it controls the interval
from 0 to 0.8 seconds. After 0.8 seconds, the final scale remains `1`.

Use the same timing for the stripe, changing rotation rather than scale:

```python
stripe.transform.rotation_degrees.keyframe(0, -70)
stripe.transform.rotation_degrees.keyframe(
    0.8, -25, interpolation=Interpolation.EASE_OUT
)
```

This snippet uses `stripe`, the layer returned when adding the rectangle in
the complete script below.

## 3. Move and fade in the text

Create the headline with `opacity=0`, then animate its position and opacity
independently:

```python
headline.transform.position.keyframe(0, (0.12, 0.53))
headline.transform.position.keyframe(
    0.8, (0.12, 0.44), interpolation=Interpolation.EASE_OUT
)
headline.opacity.keyframe(0, 0)
headline.opacity.keyframe(0.6, 1)
```

The y coordinate gets smaller, so the text moves **up**. Opacity runs from `0`
(fully transparent) to `1` (fully opaque). It finishes fading in before it
finishes moving; the two tracks need not have identical timing.

## 4. Delay the caption fade

Create the caption with `opacity=0`. Start its fade half a second later:

```python
caption.opacity.keyframe(0.5, 0)
caption.opacity.keyframe(1.1, 1)
```

Before the first keyframe, the base opacity stays at `0`. The first keyframe
marks the beginning of the fade; the second marks its end. A single keyframe
at `1.1` would make the caption jump into view at that time.

![Timeline showing circle scale and stripe rotation from 0 to 0.8 seconds, title opacity from 0 to 0.6 and position from 0 to 0.8, then caption opacity from 0.5 to 1.1. All layers hold until 3 seconds.](../assets/diagrams/first-animation-timeline.svg)

## The complete script

Save this as `first_animation.py`, alongside the first example. It is
self-contained; you do not need to import or run `first_render.py`.

[Download first_animation.py](../assets/examples/first_animation.py)

```python title="first_animation.py"
--8<-- "docs/assets/examples/first_animation.py"
```

Run it in your project environment:

```bash
uv run python first_animation.py
```

Open **`first-animation.mp4` in the current directory**. The render is 960 × 540
at 30 fps, lasts three seconds and has no audio. Running it again replaces the
same output because the example uses `overwrite=True`.

## Change the animation duration

Change the circle and stripe's destination times from `0.8` to `1.6`. Their
scale and rotation changes now take twice as long, while the text and caption
keep their original timing.

[Continue to animation and keyframes](animation.md){ .md-button .md-button--primary }

Or browse the [examples gallery](../gallery/index.md) for effects, compositing
and audio-reactive motion.
