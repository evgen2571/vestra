---
description: Map audio RMS to bounded visual properties and compare attack and release smoothing with an audible example.
---

# Make a visual respond to audio

Start with an audio clip on the project timeline, measure it, then map its
range to the property you want to move. This study uses an original generated
pulse recording so you can hear each change and reproduce it without downloads.

<figure>
  <video controls playsinline preload="metadata" poster="../../../assets/renders/audio-bounded.webp" width="640" height="360" aria-label="A mint circle grows on each audible pulse, remaining between two fixed outline circles that mark scale 0.75 and 1.30.">
    <source src="../../assets/renders/audio-bounded.mp4" type="video/mp4">
    <a href="../../assets/renders/audio-bounded.mp4">Download the bounded response</a>.
  </video>
  <figcaption>The fixed outlines mark the minimum and maximum size. Louder pulses grow the filled circle further; the release lets it settle between pulses.</figcaption>
</figure>

The [audio and signals concept](../../concepts/audio-and-signals.md) explains
why playback and analysis are separate. For a first soundtrack, see
[add audio](audio.md).

## Choose a measurement

This starts a new project with your own `assets/music.wav`, containing at
least three seconds of audio:

```python
from vestra import Circle, Project

project = Project(size=(640, 360), fps=30, duration=3)
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
smoothed_scale = bounded_scale.envelope(0.01, 0.15)

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
  <video controls playsinline preload="metadata" poster="../../../assets/renders/audio-smoothing.webp" width="640" height="360" aria-label="Three circles hear the same pulses. Raw RMS changes quickly, a 0.01 second attack and 0.20 second release follow the pulse then linger, and a 0.15 second attack reduces the response to short pulses.">
    <source src="../../assets/renders/audio-smoothing.mp4" type="video/mp4">
    <a href="../../assets/renders/audio-smoothing.mp4">Download the smoothing comparison</a>.
  </video>
  <figcaption>All three use the same RMS, remap and clamp. The middle and right share a 0.20 s release; changing only attack shows why a slower attack suppresses short pulses.</figcaption>
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
project.render("audio-response.mp4", backend="cpu", overwrite=True)
```

This replaces `audio-response.mp4`. It includes the authored audio by default.
Use `project.output_audio = False` to keep analysis while publishing a silent
video. See the [signals reference](../../reference/signals.md) for binding
operations, frequency constraints and other transforms.

## Run the audible comparisons

[Download audio_response.py](../../assets/examples/audio_response.py), install
Vestra with its native media requirements, then run:

```bash
python audio_response.py
```

The program writes `audio-pulses.wav` and replaces `audio-bounded.mp4`,
`audio-smoothing.mp4` and `audio-spectra.mp4` in the current directory. All three
are 640 × 360, 30 fps, three seconds, with sound and no speech. The third is
explained in [draw an audio spectrum](spectra.md).

```python
--8<-- "docs/assets/examples/audio_response.py"
```

## Credits and reproduction

The tones and procedural graphics are original to this documentation; the
program uses Python's standard-library `wave` module and Vestra's bundled
default font. There are no external media inputs. Previews were rendered with
CPU and encoded as H.264/yuv420p plus AAC, with fast-start metadata. Posters
sample **1.0 s** for the bounded response and **0.9 s** for smoothing and
spectra. Original-render checksums are recorded below. Encoding bytes may differ
across FFmpeg versions; these originals used FFmpeg **8.1.3**.

| Original render | SHA-256 |
| --- | --- |
| `audio-bounded.mp4` | `e0533d8999b3038a79168dd3934116e90497c62ab03c32ac6190bf5b2c144a95` |
| `audio-smoothing.mp4` | `90a7dc1d2ae3695edb7c55e5d0735d62219cc942bf10a2e64baf1c2bdd607da1` |
| `audio-spectra.mp4` | `b9e71108aca6c190edcc105d4a1ab16ce8a6377a4e95a228559b27cf0d70cadd` |

From the repository root in the native contributor environment, regenerate the
previews and posters with:

```bash
just python-sync
python scripts/render-docs-media.py --learning
```

Original renders and inspection frames stay under `target/docs-media/`.
The website build uses the committed delivery copies and does not run the
engine or download media.
