# Transitions, flashes, and presets

Transitions connect two layers in the same composition. Add the layers first,
then place a transition in that composition's transition collection. Use your
own `assets/one.png` and `assets/two.png` for this example; the layers overlap
from second two to second three.

```python
from vestra import Crossfade, Image, Project

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)

outgoing = project.root.add(Image("assets/one.png"), start=0, duration=3)
incoming = project.root.add(Image("assets/two.png"), start=2, duration=3)

project.root.transitions.add(outgoing, incoming, Crossfade(), start=2, duration=1)
```

Current reusable definitions include `Crossfade`, `DirectionalPush`,
`ZoomCrossfade`, and `ZoomBlurTransition`, along with their convenience forms.
The transition interval is in the owning composition's local seconds. The
endpoints must belong to that composition and have a positive overlap that
makes sense for the chosen effect.

Image, video, shape, text, and composition layers are direct canonical
transition endpoints. A Python `Color`/`SolidColor` layer may also be used as
an endpoint, but Vestra adapts it during lowering. Do not name a canonical
`solid_color` clip directly in a JSON transition placement.

To add a white flash across the entire frame, including both layers:

```python
from vestra import Flash

project.flashes.add(
    Flash(start=2.0, duration=0.12, colour="#ffffff", opacity=0.7, fade_out=0.08)
)
```

Presets are layer-owned reusable motion intent. They are currently intended
for image layers and a layer can hold one at a time:

```python
project = Project(
    size=(1280, 720),
    fps=30,
    duration=6,
)
photo = project.root.add(Image("assets/one.png"))
photo.presets.apply_slow_drift(start=0.5, duration=4, intensity=0.6)
```

Nested compositions can be transition endpoints in their parent composition.
Transitions authored inside the child remain child-local. Do not confuse a
high-level `CompositionLayer` with the canonical group representation produced
when the project is lowered.
