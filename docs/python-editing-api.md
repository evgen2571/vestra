# Python editing API

The normal Python entry point is the mutable `vestra.Project`. It is an editor
graph, not a JSON dictionary. A project owns one root `Composition`, a
composition owns sibling `Layer` objects, and each layer places a `Source` on
that composition's local timeline.

```python
from vestra import Project
from vestra.sources import Image

project = Project(
    size=(1920, 1080),
    fps=30,
    duration=5,
    base_directory=".",
)
scene = project.root
background = scene.add(
    Image("examples/assets/red.png", sizing="cover"),
    start=0,
    duration=5,
    z=0,
)
```

`Source` describes what is drawn. `Layer` owns placement and presentation:
`start`, `duration`, `z`, `visible`, `opacity`, `blend_mode`, `transform`,
effects, keyframes, and signal bindings. A source is copied when it is placed,
so reusing an `Image` or `Color` value does not share layer state.

Layers are composition-owned placements. `Composition.add()` and
`Composition.group()` create them; a layer cannot be reparented, cloned, or
used as a transition endpoint in another composition. To reuse content, place
the same source value in the destination composition; the existing placement
remains in its original owner. The source is copied, so the two layers have
independent source and presentation state. Layer IDs are unique only
within their composition and remain stable for that placement's lifetime.

## Nested compositions

`Composition.group()` creates a `CompositionLayer`. The returned object is a
layer in its parent and also exposes the child composition through `add()` and
`child`. Child times stay local to the group.

```python
from vestra.sources import Color, Spectrum2D

visualizer = scene.group("visualizer", start=1, duration=4, z=10)
visualizer.opacity = 0.9
visualizer.add(Color("#101018"), start=0, duration=4, z=0)
visualizer.add(
    Spectrum2D(band_count=24, min_hz=40, max_hz=16000),
    start=0.5,
    duration=3,
    z=1,
)
```

The lowering layer maps nested compositions to native groups. It allocates
composition-local IDs, preserves local timing, and deduplicates equivalent
image and audio paths within a snapshot.

## Properties, keyframes, and easing

Properties are typed handles. Assign a plain value for a static property, or
keep the handle and add local keyframes. Interpolation values are available as
`Interpolation` members; `CubicBezier` is available for a custom easing curve.

```python
from vestra import CubicBezier, Interpolation

background.opacity = 0.0
background.opacity.keyframe(0.0, 0.0)
background.opacity.keyframe(1.0, 1.0, interpolation=Interpolation.EASE_OUT)

background.transform.position = (0.5, 0.5)
background.transform.scale = 1.0
background.transform.scale.keyframe(
    0.0,
    1.0,
    interpolation=CubicBezier(0.25, 0.1, 0.25, 1.0),
)
background.transform.rotation_degrees.keyframe(0.0, -2.0)
background.transform.rotation_degrees.keyframe(5.0, 2.0)
```

`opacity` and transform scale/rotation are bindable scalar properties. Point
properties support typed values and keyframes. Crop is an image-only property.
Property keyframes are expressed in layer-local seconds, while layer and
transition starts are composition or project timeline values as described by
their owner.

## Effects

Each layer has an ordered `EffectStack`. Add descriptors, or extend it with a
sequence. Effect descriptors use the native parameter catalog and expose typed
property handles for animatable parameters.

```python
from vestra.effects import Bloom, Vignette

background.effects.add(Vignette(0.2, 1.0, 0.5, "#000000"))
bloom = background.effects.add(Bloom(threshold=0.5, radius=4.0, intensity=0.8))
bloom.intensity.keyframe(0.0, 0.0)
bloom.intensity.keyframe(1.0, 0.8, interpolation=Interpolation.EASE_OUT)
```

`project.post_effects` is another ordered stack for project-output effects.
The layer stack and root stack have different native scopes. Clip-only effects
such as `MotionBlur` and `CameraShake` cannot be added to the root stack.

## Audio and signal bindings

Audio stays track-oriented. Paths are kept as authored values until lowering,
and the high-level API handles asset registration.

```python
from vestra.audio import BassBoost

music = project.audio.track("music")
song = music.add("examples/assets/tone.wav", start=0.0, trim_end=5.0, gain=0.9)
song.fade_in = 0.5
song.fade_out = 0.5
music.effects.add(BassBoost(gain_db=3.0, frequency_hz=100.0))
```

Signals are immutable chains over the mixed master audio. The same signal can
bind to any compatible property, including effect properties and future source
properties.

```python
bass = (
    project.audio.signal.band_energy(40, 160)
    .gain(1.8)
    .remap(input=(0.0, 0.2), output=(0.0, 1.0))
    .clamp(0.0, 1.0)
    .envelope(0.025, 0.18)
)
visualizer.transform.scale.bind(bass, operation="multiply")
bloom.intensity.bind(bass, operation="replace")
```

Uniform scale bindings target both components. For independent modulation, use
the modifier-only targets `transform.position_x`, `position_y`, `scale_x`, and
`scale_y`. Their base values and keyframes remain on the parent `position` or
`scale` property.

The project must contain authored audio for a useful signal. Analysis happens
during preparation and is reused by prepared random-access frames.

## Transitions, presets, flashes, and post-effects

Transitions belong to a composition because they connect two sibling layers.
The current native transition endpoints are direct `Image` and `Group` clips.
The high-level capability adapter can lower current `Color`, `ParticleSystem`,
and `Spectrum2D` endpoints through a presentation group when their semantics
permit it.

```python
from vestra.transitions import Crossfade

incoming = scene.add(Image("examples/assets/blue.png", sizing="cover"), start=0.5, duration=4.5)
scene.transitions.add(background, incoming, Crossfade(start=0.5, duration=0.5))
```

Nested composition transitions are rejected. Cinematic presets are image-only
and layer-owned.

```python
from vestra import Flash, Preset
from vestra.effects import ColorAdjust

background.presets.add(Preset("slow_drift", intensity=0.7, duration=5.0))
project.flashes.add(Flash(2.0, 0.1, "#ffffff", opacity=0.6, fade_out=0.1))
project.post_effects.add(ColorAdjust(0.0, 1.0, 0.0, 1.0))
```

Flashes and post-effects are root-only because that is where the native model
applies them. They do not become nested composition features by implication.

## Validation, snapshots, preparation, and rendering

Mutation happens on the editor graph. Runtime operations lower a stable,
immutable snapshot.

```python
report = project.validate()
if not report.is_valid:
    for diagnostic in report.diagnostics:
        print(diagnostic.code, diagnostic.message)

snapshot = project.snapshot()
assert snapshot.to_dict()["schema_version"] == 2

prepared = project.prepare(backend="cpu")
frame = prepared.render_frame_seconds(2.0)
print(frame.width, frame.height, len(frame.to_bytes()))

result = project.render("examples/output/python-api.mp4", backend="cpu", overwrite=True)
print(result.selected_backend, result.output_path)
```

`project.render_frame(seconds)` is a convenience call that prepares for that
request. For several previews, call `prepare()` once and use
`render_frame_seconds`, `render_frame_ns`, or `render_frame_number` on the
prepared object. `project.render()` returns the native `RenderResult`, whose
`selected_backend`, optional `fallback`, adapter information, warnings, timing,
and performance fields describe what actually ran.

Prepared projects own the immutable snapshot and decoded/prepared runtime
state. Later editor mutations do not change an existing prepared object. Make
another snapshot and prepare again after changing assets, audio, or graph
values. The referenced files must remain readable for the preparation and
operation lifetime. `PreparedProject` is not a concurrent multi-render object;
serialize operations on one prepared value.

Progress callbacks receive native events before publication. The binding does
not treat a Python callback as the success authority and filters the native
post-publication `completed` event. A successful `render()` return is the
completion signal. If a callback raises, rendering is cancelled and the Python
exception remains primary; native cleanup details may be attached to it.

Backend reports are factual. `backend="cpu"` reports CPU. `backend="wgpu"`
requires an available WGPU adapter and does not silently become CPU. `auto` may
fall back before the first frame, and the returned result or preparation report
contains that fallback.

## Advanced and native layers

`vestra.authoring.ProjectBuilder` remains the supported advanced API for exact
canonical authoring, fixtures, and lowering implementation. Its objects are
builder-owned and its model is intentionally more explicit than the editor
graph.

`vestra.ProjectSnapshot` is the immutable native project type. It supports
`load`, `from_json`, `from_dict`, `to_json`, `to_dict`, and `save`. The
implementation-level `vestra._native` module contains the thin PyO3 mapping for
the Rust SDK. Normal code should use `Project`, `ProjectSnapshot`, and the
high-level runtime wrappers instead of importing `_native` directly.
