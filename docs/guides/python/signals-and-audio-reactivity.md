# Signals and audio reactivity

Use `project.audio.signal` to derive control values from the project's master
audio, then bind a signal to a visual property.

```python
from vestra import Circle, Project

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
music = project.audio.track("music")
music.add("assets/music.wav", trim_end=5)

pulse = project.root.add(Circle(radius=120, fill="#38bdf8"))
energy = project.audio.signal.band(80, 240)
```

Use your own `assets/music.wav` with at least five seconds of audio. The signal
measures energy between 80 and 240 Hz. Turn that measurement into a small,
bounded change in size before attaching it to the circle:

```python
scale_range = energy.remap(input=(0, 0.5), output=(0.85, 1.25))
bounded_scale = scale_range.clamp(0.85, 1.25)
smoothed_scale = bounded_scale.envelope(0.03, 0.2)

pulse.transform.scale.bind(smoothed_scale, operation="multiply")
```

The circle varies between 85% and 125% of its original size. A short attack
follows the beat; the longer release keeps the circle from snapping back between
beats. These ranges suit this example; adjust the input range for your recording.


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
