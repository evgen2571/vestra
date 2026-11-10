---
description: Animate a rectangle's position with three keyframes and ease in/out interpolation.
---

# Your first animation

Animate the rectangle from [your first render](first-render.md). It moves from
the left to the right over 1.5 seconds, then moves back toward the left over
the remaining 1.5 seconds.

<figure>
  <video controls playsinline preload="metadata" poster="../../assets/renders/first-animation.webp" width="640" height="360" aria-label="A mint rounded rectangle moves across a dark canvas from left to right, then back toward its initial position.">
    <source src="../assets/renders/first-animation.mp4" type="video/mp4">
    <a href="../assets/renders/first-animation.mp4">Download the animation</a>.
  </video>
  <figcaption>Expected output: a three-second position animation with no audio.</figcaption>
</figure>

The steps below replace the rectangle's static position with keyframes. Keep
the same project, background and rectangle, or use the
[complete script](#the-complete-script).

## 1. Move the rectangle to the right

Add `Interpolation` to the first import:

```python
from vestra import Interpolation, Project
```

Replace `card.transform.position = (0.25, 0.5)` with these two keyframes:

```python
card.transform.position.keyframe(0, (0.25, 0.5))
card.transform.position.keyframe(
    1.5, (0.75, 0.5), interpolation=Interpolation.EASE_IN_OUT
)
```

At zero seconds the rectangle is 25% across the canvas. At 1.5 seconds it is
75% across. Its y coordinate stays at `0.5`, so it moves horizontally. Vestra
calculates the position between the two keys.

`EASE_IN_OUT` makes the rectangle accelerate at the beginning and slow down at
the end. **Interpolation belongs to the destination keyframe**: the setting on
the key at 1.5 seconds controls the interval from 0 to 1.5 seconds. Without a
third keyframe, the rectangle would stay on the right for the rest of the video.

## 2. Add the return movement

Add a third keyframe after the first two:

```python
card.transform.position.keyframe(
    3, (0.25, 0.5), interpolation=Interpolation.EASE_IN_OUT
)
```

This defines a return to x = `0.25` at second 3. The time is measured from the
start of the layer, so the return interval lasts **1.5 seconds**, not three.
Its destination also uses ease in/out interpolation.

![Position timeline: at zero seconds x is 0.25, at 1.5 seconds x is 0.75, and at three seconds x is 0.25. Both intervals use ease in/out interpolation.](../assets/diagrams/first-animation-timeline.svg)

!!! note "The end timestamp is not a rendered frame"

    At 30 fps, a three-second video contains 90 frames at times `0`, `1/30`,
    …, `89/30` seconds. The last frame is at about **2.967 seconds**, just
    before the third keyframe. The rectangle is almost back at its initial
    position; the final frame is not an exact duplicate of the first.

## The complete script

Save this as `first_animation.py`. It is a complete copy of the quickstart
example from the repository's README showcase. You do not need to run or
import `first_render.py`.

[Download first_animation.py](https://raw.githubusercontent.com/evgen2571/vestra/develop/examples/tutorials/first_animation.py)

```python title="first_animation.py"
--8<-- "examples/tutorials/first_animation.py"
```

Run it in your project environment:

```bash
uv run python first_animation.py
```

Open **`quickstart.mp4` in the current directory**. The render is 640 × 360 at
30 fps, lasts three seconds and has no audio. The output name comes from the
script's final `project.render(...)` call, independently of the Python
filename. Running it again replaces `quickstart.mp4` because the example uses
`overwrite=True`.

## Change the timing

Move the middle keyframe from `1.5` to `1`. The rectangle now takes one second
to move right and two seconds to return. The overall duration remains three
seconds. Keep keyframe times in increasing order when adjusting them.

[Continue to animation and keyframes](animation.md){ .md-button .md-button--primary }

The [examples gallery](../gallery/index.md) includes larger projects with
effects, compositing and audio-reactive motion.

<span id="preview-reproduction"></span>
