# Signals and audio reactivity

Use `project.audio.signal` to derive control values from the project's master
audio, then bind a signal to a visual property.

```python
from vestra.sources import Circle

layer = project.root.add(
    Circle(radius=120, fill="#38bdf8"), duration=5
)
energy = project.audio.signal.band(80, 240)
reactive = energy.remap(0.0, 0.5, 0.85, 1.25).clamp(0.85, 1.25)
layer.transform.scale.bind(reactive, operation="multiply")
```

`rms()`, `peak()`, and `band(min_hz, max_hz)` create scalar signals. Signal
transforms return new signals, so keep the intermediate value when you want to
reuse it. `envelope(attack, release)` smooths changes; `response_curve(...)`
reshapes the response.

Bindable scalar properties support `replace`, `add`, and `multiply`. Transform
components also provide binding targets for position and scale components. A
signal changes evaluation values. It does not add an audio track and it does
not make a silent project audible.

Audio analysis happens during preparation. Keep the audio clip on the project
timeline that the visual layer uses, and clamp or remap the signal before
binding it to avoid extreme motion.
