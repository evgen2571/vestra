---
description: Understand Vestra property values, layer-local time and keyframe interpolation with timelines and a rendered comparison.
---

# Animation and keyframes

A keyframe records **a property value at a particular time**. Between two
keyframes, Vestra calculates the value using the interval's interpolation.
This lets you control both the endpoints of a change and its timing.

<figure>
  <video controls playsinline preload="metadata" poster="../../assets/renders/keyframes.webp" width="960" height="540" aria-label="Four dots travel the same distance in the same two seconds, using linear, ease out, ease in out and hold interpolation.">
    <source src="../assets/renders/keyframes.mp4" type="video/mp4">
    <a href="../assets/renders/keyframes.mp4">Download the interpolation comparison</a>.
  </video>
  <figcaption>Each dot travels between the same positions over two seconds. The rows compare interpolation modes; the labels identify them without relying on colour.</figcaption>
</figure>

If you have not animated a layer yet, start with the
[first animation tutorial](first-animation.md). This page explains how its
keyframe tracks are evaluated.

## Property values

A layer has properties such as position, scale, rotation and opacity. Without
keyframes, a property's **base value** applies for the whole layer:

```python
headline.opacity = 0.5
headline.transform.position = (0.5, 0.5)
```

`opacity` is a single number. `position` is a pair: x across the composition,
y down it, using normalized coordinates. For example, `(0.5, 0.5)` is the
centre. Both position components follow the same interpolation curve.

Keyframes belong to the property on the **placed layer**, returned by
`project.root.add(...)`. Animate a shape's scale to make it grow; the static
source dimensions such as `Ellipse(width=210, height=210)` are not themselves
property tracks.

## Position keyframes

In the tutorial, these two keyframes move the heading upward:

```python
headline.transform.position.keyframe(0, (0.12, 0.53))
headline.transform.position.keyframe(
    0.8,
    (0.12, 0.44),
    interpolation=Interpolation.EASE_OUT,
)
```

The two position values define the endpoints. `0.8` is an **absolute time on
the layer's clock**, not a duration measured from the previous keyframe.
Import the enum with `from vestra import Interpolation`.

### Interpolation belongs to the destination keyframe

In Vestra, `interpolation=` controls the interval **ending at that keyframe**.
The interval from A to B uses B's interpolation, and the interval from B to C
uses C's:

![Two intervals: keyframe A at zero, keyframe B at 0.8 seconds with ease out governing A to B, and keyframe C at 1.6 seconds with linear governing B to C.](../assets/diagrams/keyframe-intervals.svg)

For three position keyframes:

```python
headline.transform.position.keyframe(0, (0.12, 0.53))
headline.transform.position.keyframe(
    0.8, (0.12, 0.44), interpolation=Interpolation.EASE_OUT
)
headline.transform.position.keyframe(
    1.6, (0.30, 0.44), interpolation=Interpolation.LINEAR
)
```

The first interval eases out **into** the key at 0.8 seconds. The second interval
moves at a constant rate **into** the key at 1.6 seconds. Setting easing only
on the first keyframe does not ease the interval after it.

### What happens outside the keyframes?

| When | The value Vestra uses |
| --- | --- |
| Before the first keyframe | The property's base value; there is no automatic interpolation into the first key. |
| At a keyframe | That keyframe's value. |
| Between two keyframes | An interpolated value using the **later** key's interpolation. |
| After the final keyframe | The final keyframe's value, for the rest of the layer. |

For example, a delayed caption fade needs **two** keyframes:

```python
caption.opacity = 0
caption.opacity.keyframe(0.5, 0)
caption.opacity.keyframe(1.1, 1)
```

It stays invisible before 0.5 seconds, fades from 0.5 to 1.1, then stays fully
visible. A lone keyframe at `1.1` jumps from the base value to `1` at that time.
Changing the base value later does not remove existing keyframes; use
`caption.opacity.clear_keyframes()` when returning a property to a static value.

## Interpolation modes

Play the comparison above. Each dot travels from x `0.38` to `0.88` between
0.3 and 2.3 seconds. Only the destination's interpolation changes.

| Interpolation | What you see | A useful starting point for |
| --- | --- | --- |
| `LINEAR` | Constant speed. This is the default when omitted. | A progress indicator or steady pan. |
| `EASE_IN` | Starts slowly, then accelerates. | A layer leaving the scene. |
| `EASE_OUT` | Starts quickly, then slows down. | Text moving into its final position. |
| `EASE_IN_OUT` | Smooth start and finish. | Moving between two resting positions. |
| `HOLD` | Keeps the previous value, then jumps at the destination time. | A step change, cut or discrete state. |

These built-in curves have defined formulas: `t`, `t²`, `1 − (1 − t)²` and
`t²(3 − 2t)` for linear, ease in, ease out and ease in/out respectively, where
`t` is progress from zero to one through the interval. They are not the CSS
curves with similarly named keywords.

??? note "A custom easing curve"

    Pass a `CubicBezier` in place of an `Interpolation` member:

    ```python
    from vestra import CubicBezier

    headline.transform.position.keyframe(
        0.8,
        (0.12, 0.44),
        interpolation=CubicBezier(0.42, 0, 0.58, 1),
    )
    ```

    All four controls must be finite; x controls must be between 0 and 1.
    Y controls can go outside that range, which can overshoot the endpoint
    values. Keep the result valid for the property you are animating. See the
    [interpolation reference](../concepts/effects-transitions-and-animation.md#interpolation)
    for the canonical JSON form and validation rules.

[Download the runnable comparison](../assets/examples/keyframes.py), then run
`uv run python keyframes.py` to create `keyframes.mp4`. It uses the same
installation as the tutorials, has no external assets or audio, and replaces
that filename on each run. The complete source is below.

??? example "Complete comparison source"

    ```python title="keyframes.py"
    --8<-- "docs/assets/examples/keyframes.py"
    ```

## Layer-local time

Layer keyframes use **seconds since that layer starts**, in its owning
composition. Moving a layer later on the timeline moves its whole animation
with it:

```python
caption = project.root.add(
    Text("Delayed caption", font_size=28),
    start=2,
    duration=2,
    opacity=0,
)
caption.opacity.keyframe(0, 0)
caption.opacity.keyframe(0.5, 1)
```

In a root composition, that fade happens from project second **2 to 2.5**.
The layer is visible from second 2 until just before second 4, so use a project
long enough to contain it. Inside a nested composition, the same rule applies
relative to that composition. Effects with their own active intervals use
**effect-local** keyframe time; see
[effect ownership](../guides/python/animation.md#effect-ownership).

### Seconds and frames are different

`fps=30` samples the animation 30 times a second; it does not make keyframe times
frame numbers. A keyframe at `0.8` seconds falls on frame 24 at 30 fps.
A one-second video contains frames at `0`, `1/30`, …, `29/30` seconds;
the end time `1.0` is excluded.

Put the final keyframe before the end of the layer if the viewer should see
the final value. In the tutorial, the last fade ends at 1.1 seconds. The
composition then stays unchanged until 3 seconds.

## Track validation

- Add keyframes in **strictly increasing** order, with non-negative times.
  Duplicate timestamps are invalid; adding a keyframe does not replace an
  existing one at that time.
- Use the property's value type and supported range: opacity is `0` to `1`;
  scale components must be positive. Fade out with opacity rather than scaling
  all the way to zero.
- Run `project.validate()` to check structural constraints. An invalid report
  contains diagnostics; rendering also validates before producing output.
  Validation alone does not check media files or FFmpeg.

For the low-level forms, see
[the animation reference](../concepts/effects-transitions-and-animation.md#interpolation)
and [the project format](../reference/project-format.md). For a practical next
step, explore [effects](../guides/python/effects.md) or find a project in the
[examples gallery](../gallery/index.md).
