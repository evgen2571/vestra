# Add audio

Create a track on the project's audio timeline, then place path-based clips on
that track.

```python
track = project.audio.track("music")
clip = track.add(
    "assets/music.wav",
    start=0,
    trim_start=4,
    trim_end=14,
    gain=0.8,
    fade_in=0.25,
    fade_out=0.5,
)
track.gain = 0.9
project.output_audio = True
```

Clip, track, and master audio effects have different scopes. For example,
`PlaybackSpeed` is clip-scoped, while `BassBoost` can be used on a clip, track,
or master stack:

```python
from vestra import BassBoost, PlaybackSpeed

clip.effects.add(PlaybackSpeed(1.05))
project.audio.effects.add(BassBoost(gain_db=3.0, frequency_hz=120.0))
```

Audio uses project seconds for clip placement, but not every field is in that
domain. `start` is project time. `trim_start` and `trim_end` are source-media
positions. `fade_in` and `fade_out` are clip-relative durations. Gain
automation keyframe times are clip-local.

Use `project.audio.crossfade(outgoing, incoming)` only after the clips overlap
and the outgoing clip has an explicit trim end. `project.render(...)` includes
authored audio when the output-audio policy permits it. `None` follows whether
the project contains audio clips; set `True` or `False` to choose explicitly.
