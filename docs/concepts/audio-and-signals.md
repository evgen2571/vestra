# Audio and signals

Audio playback and audio-derived control data are related, but they are not
the same thing.

The high-level project owns an `AudioTimeline`. It contains tracks, and tracks
contain path-based audio clips placed on project time. Clips have trim points,
gain, fades, optional gain automation, and clip-scoped effects. Tracks and the
master timeline can have their own gain and effects too.

An audio signal is a scalar description of analysis data. `project.audio.signal`
can create RMS, peak, or frequency-band signals. Signal methods such as
`gain`, `remap`, `clamp`, `envelope`, and `response_curve` build a new immutable
signal with additional transforms.

A signal has no visible result by itself. Bind it to a bindable visual property
or a component of a transform:

```python
energy = project.audio.signal.band(80, 240)
layer.opacity.bind(energy.clamp(0, 1), operation="replace")
```

During evaluation, Vestra analyzes the authored audio, samples the signal at
the current project time, applies its transforms, and combines the value with
the bound property's base value according to the binding operation. The audio
clip still plays independently. Removing the binding leaves playback intact;
removing the audio removes the signal's source data.

Keep signal ranges in mind. Use remapping, clamping, and smoothing before
binding a signal to a sensitive property. Audio analysis is part of preparation
for a render, so a project that uses audio reactivity must have usable audio
inputs even when its visual output is the main goal.
