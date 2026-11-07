---
description: Map audio RMS to bounded visual properties and see how attack and release change the response.
---

# Make a visual respond to audio

Start with an audio clip on the project timeline, measure it, then map its
range to the property you want to move. This study uses an original generated
bass-and-melody phrase. The preview is silent: its waveform and moving marker
show when the input changes. The audio clips remain in the project for analysis.

<figure>
  <video controls playsinline preload="metadata" poster="../../../assets/renders/audio-bounded.webp" width="640" height="560" aria-label="Silent study: a waveform marks five input beats. A mint circle grows between fixed minimum and maximum rings, and a moving marker follows the measured scale curve.">
    <source src="../../assets/renders/audio-bounded.mp4" type="video/mp4">
    <a href="../../assets/renders/audio-bounded.mp4">Download the bounded response</a>.
  </video>
  <figcaption>The rings mark scale 0.75 and 1.30. The lower curve traces the same multiplier, rising with each beat and settling during release. Follow the marker against the waveform to see the timing.</figcaption>
</figure>

The [audio and signals concept](../../concepts/audio-and-signals.md) explains
why playback and analysis are separate. For a first soundtrack, see
[add audio](audio.md).

## Choose a measurement

This starts a new project with your own `assets/music.wav`, containing at
least three seconds of audio:

```python
from vestra import Circle, Project

project = Project(
    size=(640, 360),
    fps=30,
    duration=3,
)

project.audio.track("music").add("assets/music.wav", trim_end=3)

pulse = project.root.add(Circle(radius=70, fill="#9edbc7"))

energy = project.audio.signal.rms()
```

RMS describes overall level and suits a breathing or pulsing scale. Use
`peak()` for brief amplitude peaks, or `band(80, 240)` to respond to a bass
range instead of all frequencies. These read the master mix after audio
processing; they are not per-track taps.

## Give the visual a safe range

Continue the same project:

```python
scale_response = energy.remap(input=(0, 0.18), output=(0.75, 1.30))
bounded_scale = scale_response.clamp(0.75, 1.30)
smoothed_scale = bounded_scale.envelope(0.01, 0.20)

pulse.transform.scale.bind(smoothed_scale, operation="multiply")
```

The remap maps an RMS of zero to `0.75` and `0.18` to `1.30`. Higher levels
would extrapolate beyond `1.30`, so the clamp is deliberate. `multiply` keeps
the layer's authored scale as the baseline. If the authored scale is `2`, the
resulting range is `1.50`–`2.60`, before other modifiers.

`0.18` fits the generated recording; it is not a universal normalization value.
For another recording, begin with a modest visual range and adjust the input
maximum until typical loud sections use most of it. If every beat reaches the
same maximum, raise the input maximum. If changes are barely visible, lower it.
Keep the clamp while tuning.

## Compare attack and release

<figure>
  <video controls playsinline preload="metadata" poster="../../../assets/renders/audio-smoothing.webp" width="640" height="560" aria-label="Silent comparison: three measured response curves share one waveform and playhead. Raw RMS has sharp peaks, fast attack retains a tail, and slow attack reduces each short peak.">
    <source src="../../assets/renders/audio-smoothing.mp4" type="video/mp4">
    <a href="../../assets/renders/audio-smoothing.mp4">Download the smoothing comparison</a>.
  </video>
  <figcaption>Read all three rows on the same 0–1 vertical scale. The two smoothed rows share a 0.20 s release: the fast attack rises sharply, while the slow attack reduces short peaks. Both retain a tail after raw RMS returns to zero.</figcaption>
</figure>

Attack controls movement toward a rising value; release controls movement
toward a falling value. These are smoothing time constants in seconds, not
a fixed delay or a promise to reach the target exactly at that time. A zero
duration follows that direction immediately. “No envelope” still uses the
analysis window of the RMS measurement.

For a sharp beat, try a short attack such as `0.01`–`0.03` s. A longer release
such as `0.15`–`0.30` s carries motion between beats. Increase attack to ignore
brief transients; increase release to retain a longer tail. Transforms run
in call order: this example bounds the target before smoothing it.

To finish the small project above:

```python
project.output_audio = False

project.render("audio-response.mp4", backend="cpu", overwrite=True)
```

This replaces `audio-response.mp4` with a silent video. Setting
`output_audio = False` retains the clips and prepared analysis; changing it to
`True` publishes the same visuals with sound. See the
[signals reference](../../reference/signals.md) for binding operations, frequency
constraints and other transforms.

<span id="run-the-audible-comparisons"></span>
<span id="credits-and-reproduction"></span>

## Complete example

[Download audio_response.py](../../assets/examples/audio_response.py) and run:

```bash
python audio_response.py
```

The program creates `audio-pulses.wav` with Python's standard-library `wave`
module and replaces `audio-bounded.mp4`, `audio-smoothing.mp4` and
`audio-spectra.mp4` in the current directory. The three four-second videos are
silent; the generated WAV is their analysis input. The spectrum study is
explained in [draw an audio spectrum](spectra.md).

The curves come from actual prepared Vestra signal values sampled at 30 fps;
the moving dots are bound directly to those signals. The input waveform comes
from the WAV samples. This makes the plots useful when playback is paused as
well as while it runs. See
[preview reproduction](../../development/documentation.md#reproduce-visual-learning-studies)
for contributor checks and regeneration.
