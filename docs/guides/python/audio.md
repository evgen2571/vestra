---
description: Place and trim audio, add fades, choose processing scopes, crossfade clips, and remix a stereo phrase.
---

# Add audio

Place a clip on the project's audio timeline, select the part of its source
you want, then adjust its sound. Rendering includes authored audio by default.
Audio placement is independent of visual layers: moving a title does not move
its soundtrack.

## Place and trim a clip

Start `soundtrack.py` with your own `assets/music.wav`. The file must contain
at least fourteen seconds of audio. This project plays source seconds 4–14
starting at project second 2, after two seconds of silence:

The render calls below use `overwrite=True`; rerunning a script replaces its
named output files.

```python
from vestra import Project, Text

project = Project(
    size=(640, 360),
    fps=30,
    duration=12,
    background="#101c24",
)

project.root.add(Text("Soundtrack excerpt", font_size=32, fill="#eff4ec"))

music = project.audio.track("music")
soundtrack = music.add(
    "assets/music.wav",
    start=2,
    trim_start=4,
    trim_end=14,
)
```

## Keep the clocks explicit

![Source seconds 4, 9 and 14 align with project seconds 2, 7 and 12 and clip-local seconds 0, 5 and 10. Below, clips pass through their own processing, their track and the master mix before branching to output audio and visual analysis.](../../assets/diagrams/audio-scopes.svg)

Changing `start` moves the excerpt on the project timeline. Changing a trim
boundary selects different source audio. Before any playback-speed effect,
project second 7 therefore reads source second 9, five seconds into the clip.

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

## Set levels and fades

Continue `soundtrack.py` with the same clip and track:

```python
soundtrack.gain = 0.8
music.gain = 0.9
soundtrack.fade_in = 0.25
soundtrack.fade_out = 0.5
```

Clip gain `0.8` applies before track gain `0.9`. The quarter-second fade in and
half-second fade out are within the selected excerpt. Keep headroom when mixing
several clips: gains multiply through the processing scopes, and overlapping
clips add together.

## Process the intended scope

| Attach effects to | What changes |
| --- | --- |
| `soundtrack.effects` | This clip only. |
| `music.effects` | All clips mixed on the music track. |
| `project.audio.effects` | All tracks in the master mix, including clips added later. |

Optionally continue the same project with clip playback speed and master EQ:

```python
from vestra import BassBoost, PlaybackSpeed

soundtrack.effects.add(PlaybackSpeed(rate=1.05))
project.audio.effects.add(BassBoost(gain_db=3.0, frequency_hz=120.0))
```

`PlaybackSpeed` makes this clip shorter; the explicit twelve-second project
keeps its duration, leaving a short silent tail. The master `BassBoost` affects
every track. Effects run in authored stack order. Add a treatment at the scope
you intend rather than accumulating it at all three scopes.

## Publish sound or retain analysis only

Finish `soundtrack.py` with:

```python
project.render("soundtrack.mp4", backend="cpu", overwrite=True)
```

```bash
uv run python soundtrack.py
```

This writes the twelve-second `soundtrack.mp4` in the current directory and
replaces it on a rerun. `Project(..., output_audio=None)` follows whether clips
exist. Set `project.output_audio = True` or `False` to choose explicitly.
Disabling publication retains the prepared master mix needed by
[signals](signals-and-audio-reactivity.md) and [spectrum sources](spectra.md).

## Crossfade two clips

An audio crossfade blends overlapping clips; it is separate from a visual
transition. Start a new `audio_crossfade.py` using the same required
`assets/music.wav`. This example has no playback-speed effects:

```python
from vestra import Project

project = Project(size=(640, 360), fps=30, duration=10)
music = project.audio.track("music")

outgoing = music.add("assets/music.wav", start=0, trim_start=0, trim_end=6)
incoming = music.add("assets/music.wav", start=4, trim_start=6, trim_end=12)
project.audio.crossfade(outgoing, incoming)

project.render("audio-crossfade.mp4", backend="cpu", overwrite=True)
```

```bash
uv run python audio_crossfade.py
```

This writes `audio-crossfade.mp4` in the current directory and replaces it on a
rerun. Its picture is the default black background. The outgoing clip ends at
project second 6; the incoming starts at second 4, so their two-second overlap
becomes matching fade-out and fade-in durations.

The helper does not move clips or add a visual transition. It needs an existing
positive overlap and an explicit outgoing `trim_end`; conflicting fade settings
are rejected. It calculates overlap from authored trim spans, so do not use it
to infer overlap after a duration-changing playback-speed effect. See
[audio contracts](../../reference/audio.md#crossfades).

## Remix a stereo phrase

[Download nightcore_remix.py](https://raw.githubusercontent.com/evgen2571/vestra/develop/examples/tutorials/nightcore_remix.py).
It generates a 100 BPM stereo phrase, remixes it, and draws a spectrum from
the processed master. The command replaces `nightcore-remix.mp4` in the
current directory.

```bash
uv run python nightcore_remix.py
```

Add `--bass-boost` for a 2 dB lift near 110 Hz. Listen for the shorter,
higher-pitched phrase, a quiet room sound on the main track, and two dark
echoes after the final note.

- `PlaybackSpeed(rate=1.10, link_pitch=True)` raises the tempo to 110 BPM
  and shifts pitch by the same ratio. A 2.4-second source clip plays in
  about 2.18 seconds.
- `RoomReverb(decay_seconds=0.5, mix=0.03)` adds a quiet room tail to the
  music track. The three-second project leaves space for the tails.
- The last note starts at 1.8 seconds in the source. Trim it to a separate
  clip, use the same linked speed, and place it at `1.8 / 1.10` seconds to
  align both clips. Put the delay on the throw track with `mix=1.0` so that
  track contributes only repeats. Track gain controls their level without
  affecting the main track. An eighth note is `30_000 / (100 * 1.10)` ms,
  or about 273 ms here.

```python
--8<-- "examples/tutorials/nightcore_remix.py"
```

## Edit into a chorus

[Download phrase_remix.py](https://raw.githubusercontent.com/evgen2571/vestra/develop/examples/tutorials/phrase_remix.py).
It generates a 120 BPM stereo mix and renders the edits with a spectrum driven
by the processed master. Rerunning the command replaces `phrase-remix.mp4`.

```bash
uv run python phrase_remix.py
```

- Raise the lead-in's high-pass cutoff from 20 to 900 Hz so the bass recedes.
  Stack the beat repeat after the sweep so the repeat plays the filtered slice.
- At 120 BPM, an eighth note lasts `30 / BPM` seconds (0.25 s), and a sixteenth
  lasts `15 / BPM` seconds (0.125 s). Play the first sixteenth of the last beat
  four times to fill the half-second before the chorus.
- Place the chorus on a new clip at 2 seconds to bring back the full mix.
  Shift it up two semitones while keeping the beat positions.
- In the next clip, slow the tape from 3.25 to 3.625 seconds, then let it stay
  silent through the end of the clip. The separate clip at 4 seconds brings
  back the downbeat.

See the [audio effect reference](../../reference/audio.md#effects) for parameter
ranges and clip-local timing.

```python
--8<-- "examples/tutorials/phrase_remix.py"
```

## Try an example with no external recording

The [audio response program](https://raw.githubusercontent.com/evgen2571/vestra/develop/examples/tutorials/audio_response.py)
generates its own bass-and-melody phrase and renders silent videos with a
waveform, playhead and measured response curves:

```bash
uv run python audio_response.py
```

It replaces `audio-pulses.wav`, `audio-bounded.mp4`, `audio-smoothing.mp4` and
`audio-spectra.mp4` in the current directory. Continue with the
[audio-reactivity guide](signals-and-audio-reactivity.md) to make a visual
respond to that recording.
