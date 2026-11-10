---
description: Place audio clips, select source excerpts, set fades and processing scopes, and control published audio.
---

# Add audio

Place a path-based clip on a track in the project's audio timeline. Rendering
includes the authored audio by default. Audio placement is independent of visual
layer placement: moving a title does not move its soundtrack.

This example plays a ten-second excerpt from your own `assets/music.wav`.
The file must contain at least fourteen seconds of audio:

```python
from vestra import Project, Text

project = Project(
    size=(640, 360),
    fps=30,
    duration=10,
    background="#101c24",
)

project.root.add(Text("Soundtrack excerpt", font_size=32, fill="#eff4ec"))

music = project.audio.track("music", gain=0.9)
soundtrack = music.add(
    "assets/music.wav",
    start=0,
    trim_start=4,
    trim_end=14,
    gain=0.8,
    fade_in=0.25,
    fade_out=0.5,
)
```

The source excerpt starts at second four and ends at second fourteen. It plays
from project second zero. Clip gain `0.8` applies before track gain `0.9`;
the quarter-second fade in and half-second fade out are within that selected
excerpt. Keep headroom when mixing several clips: gains multiply through clip,
track and master processing, and overlapping clips add together.

## Keep the clocks explicit

| Value | Meaning |
| --- | --- |
| `start=2` | Place this clip at project second 2. |
| `trim_start=4`, `trim_end=14` | Read source seconds 4–14. |
| `fade_in=0.25` | Fade over the first quarter-second of the selected clip. |
| Gain-automation keyframe time | Time since the selected clip starts, rather than project time or source-file position. |

At normal playback speed, an excerpt's duration is `trim_end - trim_start`.
A clip-scoped playback-speed effect changes that output duration. See
[the audio reference](../../reference/audio.md) for supported effects and
validation limits.

## Process the intended scope

Continue the project above:

```python
from vestra import BassBoost, PlaybackSpeed

soundtrack.effects.add(PlaybackSpeed(rate=1.05))
project.audio.effects.add(BassBoost(gain_db=3.0, frequency_hz=120.0))
```

`PlaybackSpeed` affects only this clip and changes its duration. The master
`BassBoost` affects all tracks mixed into the project, including clips you add
later. A track's `effects` collection lies between those two scopes. Effects
run in authored stack order; don't add the same treatment at all three scopes
unless that accumulation is intentional.

## Publish sound or retain analysis only

Render the project:

```python
project.render("soundtrack.mp4", backend="cpu", overwrite=True)
```

This replaces `soundtrack.mp4`. `Project(..., output_audio=None)` follows
whether clips exist. Set `project.output_audio = True` or `False` to choose
explicitly. Disabling publication does not remove the audio needed by
[signals](signals-and-audio-reactivity.md) or [spectrum sources](spectra.md).
Those analyze the prepared master mix.

An audio crossfade is different from a visual transition. At normal playback
speed, place two clips with an existing positive overlap and give the outgoing
clip an explicit `trim_end`, then call
`project.audio.crossfade(outgoing, incoming)`. The helper assigns matching
fade-out and fade-in durations; it does not move clips or add a visual
transition. It rejects conflicting fade settings. Its overlap calculation uses
authored trim spans, so do not use it to infer overlap after a duration-changing
playback-speed effect. See [audio contracts](../../reference/audio.md#crossfades).

## Try an example with no external recording

The [audio response program](https://raw.githubusercontent.com/evgen2571/vestra/develop/examples/tutorials/audio_response.py) generates
its own bass-and-melody phrase and renders silent videos with a waveform,
playhead and measured response curves. Run
`python audio_response.py`; it replaces `audio-pulses.wav` and three short
MP4s in the current directory. The
[audio-reactivity guide](signals-and-audio-reactivity.md) explains the results
and links the complete source.
