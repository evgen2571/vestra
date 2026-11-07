---
description: Draw the same prepared audio frequency bands in linear and radial Spectrum2D layouts.
---

# Draw an audio spectrum

Use `Spectrum2D` when you want to draw a distribution of frequencies rather
than drive one property with a scalar level. This comparison gives both layouts
the same generated phrase, 16 logarithmic bands, frequency range, sensitivity
and smoothing. The preview is silent; the waveform and moving marker show the
input timing.

<figure>
  <video controls playsinline preload="metadata" poster="../../../assets/renders/audio-spectra.webp" width="640" height="560" aria-label="Silent spectrum comparison: a waveform marks input beats while matching frequency bands grow upward in a linear chart and outward in a radial chart.">
    <source src="../../assets/renders/audio-spectra.mp4" type="video/mp4">
    <a href="../../assets/renders/audio-spectra.mp4">Download the spectrum comparison</a>.
  </video>
  <figcaption>A low bass tone repeats while the melody rises. Lower bands begin at the left of the linear chart and at twelve o’clock in the radial chart, proceeding clockwise. Matching band colours identify frequency, not loudness; bar length shows the response.</figcaption>
</figure>

Unlike a shape's scale binding, the spectrum requests master frequency analysis
directly. Its smoothing belongs to the source. See
[audio and signals](../../concepts/audio-and-signals.md) for the complete path
from a clip to either kind of visual.

## Place a linear spectrum

This starts a new project with your own `assets/music.wav`, containing at
least four seconds of audio:

```python
from vestra import Project, Spectrum2D
from vestra.sources import (
    Spectrum2DGradient,
    Spectrum2DLinearLayout,
    Spectrum2DRadialLayout,
)

project = Project(size=(640, 440), fps=30, duration=4, background="#101c27")
project.audio.track("music").add("assets/music.wav", trim_end=4)
project.output_audio = False

linear_source = Spectrum2D(
    band_count=16,
    min_hz=80,
    max_hz=2400,
    sensitivity=250,
    attack_seconds=0.01,
    release_seconds=0.18,
    x=0.05,
    y=0.44,
    width=0.475,
    height=0.40,
    layout=Spectrum2DLinearLayout(anchor="bottom", band_mapping="forward"),
    colour="#91e2ca",
    gradient=Spectrum2DGradient("#91e2ca", "#bdb4ff", "across_bands"),
    bar_gap_ratio=0.24,
    min_bar_height_ratio=0.015,
)
project.root.add(linear_source)
```

`x`, `y`, `width` and `height` define a normalized rectangle in the composition.
A bottom anchor grows bars upward from that rectangle's bottom edge. Forward
mapping places lower frequency bands first; reverse mapping reverses their
order. `center_out` is available for a mirrored linear arrangement.

The input is the mixed master audio. Adding another track, muting a clip, or
changing master EQ can change the spectrum. It does not analyze only the clip
nearest to it in the Python source.

## Change the geometry, keep the analysis choices

Continue the project with a radial version beside it:

```python
radial_source = Spectrum2D(
    band_count=16,
    min_hz=80,
    max_hz=2400,
    sensitivity=250,
    attack_seconds=0.01,
    release_seconds=0.18,
    x=0.59,
    y=0.41,
    width=0.36,
    height=0.52,
    layout=Spectrum2DRadialLayout(inner_radius_ratio=0.44),
    colour="#91e2ca",
    gradient=Spectrum2DGradient("#91e2ca", "#bdb4ff", "across_bands"),
    bar_gap_ratio=0.24,
    min_bar_height_ratio=0.015,
)
project.root.add(radial_source)
project.render("spectra.mp4", backend="cpu", overwrite=True)
```

The radial rectangle is approximately square in output pixels, despite its
unequal normalized width and height. `inner_radius_ratio=0.44` leaves an empty
centre; the default outward direction extends bars from that inner radius.
Set `start_angle_degrees` and `sweep_angle_degrees` for an arc, or choose
`direction="inward"` or `"both"` for a different growth direction.

## Adapt the response to a recording

| Setting | What to adjust |
| --- | --- |
| Frequency range | Focus on bass or treble, or widen it to show more of the mix. `max_hz` must fit prepared audio Nyquist. |
| `band_count` | More bands show finer frequency divisions but narrower bars; the current maximum is 48. |
| `sensitivity` | Increase it when bars barely move; lower it when many bars stay at full height. The example's 250 is tuned to the generated phrase. |
| Attack / release | Short attack follows onsets; longer release keeps bars legible between pulses. These are separate from scalar-signal `envelope`. |
| `min_bar_height_ratio` | Keep inactive bands visible as a baseline, or use zero to hide silent bars. |
| Layout and gaps | Change readability and geometry without changing the input recording. |

For a full-band recording, begin with fewer bands and a moderate sensitivity,
then tune at its quiet and loud sections. Do not assume the settings of this
sparse tone recording suit mastered music. The
[Spectrum2D reference](../../reference/sources/spectrum2d.md) lists defaults,
limits, gradients and presets.

<span id="reproduce-this-comparison"></span>

## Complete example

[Download audio_response.py](../../assets/examples/audio_response.py) and run
`python audio_response.py`. It creates a four-second bass-and-melody WAV and
replaces `audio-spectra.mp4`, plus the two
[scalar response studies](signals-and-audio-reactivity.md). The clips stay in
the project for analysis, while `output_audio = False` makes the videos silent.
Change that flag to publish sound with the same visuals.

The preview is an actual CPU render. WGPU source dispatch exists, but full
visual parity for Spectrum2D is not verified. See
[preview reproduction](../../development/documentation.md#reproduce-visual-learning-studies)
for contributor checks and regeneration.
