# Animate properties

Use keyframes to move a photo and fade it in, or change a property on an
attached effect. If keyframes are new, start with
[how animation works](../../getting-started/animation.md).

## Move and fade a photo

Save this new script as `animate_photo.py` and supply your own
`assets/photo.png`. Positions are fractions of the canvas width and height;
keyframe times are seconds since this photo layer starts.

The render calls below use `overwrite=True`; rerunning a script replaces its
named output files.

```python
from vestra import Image, Project

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)

photo = project.root.add(Image("assets/photo.png"), opacity=0)

photo.transform.position.keyframe(0, (0.35, 0.5))
photo.transform.position.keyframe(2, (0.65, 0.5))
photo.transform.position.keyframe(4, (0.5, 0.5))
photo.opacity.keyframe(0, 0.0)
photo.opacity.keyframe(0.5, 1.0)

project.render("animated-photo.mp4", backend="cpu", overwrite=True)
```

```bash
uv run python animate_photo.py
```

This writes `animated-photo.mp4` in the current directory and replaces it on a
rerun. The photo moves right for two seconds, returns over the next two, and
holds its final position for the remaining second.

Before the first keyframe, the base value applies. Between keyframes, Vestra
interpolates using the segment's ending keyframe interpolation. At and after
the final keyframe, Vestra holds the final keyframe value. Each keyframe can
use an `Interpolation` value or a `CubicBezier` easing curve. Keep keyframe
times non-negative and ordered in the time space of the property owner.
See [interpolation](../../concepts/effects-transitions-and-animation.md#interpolation)
for the available modes and Python/JSON examples.

Animation can be combined with effects and signal bindings. For a signal,
decide whether the signal should replace, add to, or multiply the base and
animated value. For a static edit, set the property directly and leave its
keyframes empty.

Copying a property keeps the two properties independent. See
[property assignment](../../reference/python-api.md#property-assignment)
for how values, keyframes, bindings and validation are copied.

## Effect ownership

You can keyframe an effect before attaching it. `layer.effects.add(effect)`
copies its current values, keyframes and signal bindings, and returns the
attached copy. Later edits to the original do not change that copy; use the
returned handle to edit the attached effect. `project.post_effects.add()` uses
the same copy-on-attachment behavior. Start a separate, asset-free
`animate_blur.py` for this example:

```python
from vestra import GaussianBlur, Project, Rectangle

project = Project(
    size=(320, 180),
    fps=30,
    duration=2,
)

card = project.root.add(Rectangle(width=160, height=90, fill="#ffffff"))

template = GaussianBlur(radius=0)
template.radius.keyframe(0, 0)
template.radius.keyframe(1, 8)
attached_blur = card.effects.add(template)
```

Clearing the template's keys leaves the attached animation intact. Edit the
returned handle when you want to change the effect on this card:

```python
template.radius.clear_keyframes()
attached_blur.radius.keyframe(2, 0)

project.render("animated-blur.mp4", backend="cpu", overwrite=True)
```

```bash
uv run python animate_blur.py
```

This writes `animated-blur.mp4` in the current directory and replaces it on a
rerun. Combine the two effect blocks in that file; they use a new project rather
than the photo above.

Common mistakes are putting a child-composition time on the parent timeline
and editing the original effect when you meant to edit its attached copy.
