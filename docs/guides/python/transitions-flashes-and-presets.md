# Transitions, flashes, and presets

Transitions connect two layers in the same composition. Add the layers first,
then place a transition in that composition's transition collection.

```python
from vestra import Crossfade, DirectionalPush
from vestra.sources import Image

outgoing = project.root.add(Image("assets/one.png"), start=0, duration=3)
incoming = project.root.add(Image("assets/two.png"), start=2, duration=3)
project.root.transitions.add(
    outgoing, incoming, Crossfade(), start=2, duration=1
)
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

Add a short root flash when a colour overlay is the right abstraction:

```python
from vestra import Flash

project.flashes.add(Flash(2.0, 0.12, "#ffffff", opacity=0.7, fade_out=0.08))
```

Presets are layer-owned reusable motion intent. They are currently intended
for image layers and a layer can hold one at a time:

```python
layer.presets.apply_slow_drift(start=0.5, duration=4, intensity=0.6)
```

Nested compositions can be transition endpoints in their parent composition.
Transitions authored inside the child remain child-local. Do not confuse a
high-level `CompositionLayer` with the canonical group representation produced
when the project is lowered.
