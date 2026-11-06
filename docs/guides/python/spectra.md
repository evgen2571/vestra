---
description: Draw the same prepared audio frequency bands in linear and radial Spectrum2D layouts.
---

# Draw an audio spectrum

Use `Spectrum2D` when you want to draw a distribution of frequencies rather
than drive one property with a scalar level. This comparison gives both layouts
the same recording, 16 bands, frequency range, sensitivity and smoothing.

<figure>
  <video controls playsinline preload="metadata" poster="../../../assets/renders/audio-spectra.webp" width="640" height="360" aria-label="Audible synthesized pulses create matching frequency-band changes in a linear spectrum on the left and a radial spectrum on the right.">
    <source src="../../assets/renders/audio-spectra.mp4" type="video/mp4">
    <a href="../../assets/renders/audio-spectra.mp4">Download the spectrum comparison</a>.
  </video>
  <figcaption>The low tone recurs in every pulse while the higher tone changes. The same frequency groups extend upward on the left and outward on the right.</figcaption>
</figure>

Unlike a shape's scale binding, the spectrum requests master frequency analysis
directly. Its smoothing belongs to the source. See
[audio and signals](../../concepts/audio-and-signals.md) for the complete path
from a clip to either kind of visual.

## Place a linear spectrum

This starts a new project with your own `assets/music.wav`, containing at
least three seconds of audio:

```python
from vestra import Project, Spectrum2D
from vestra.sources import Spectrum2DLinearLayout, Spectrum2DRadialLayout

project = Project(size=(640, 360), fps=30, duration=3, background="#101c24")
project.audio.track("music").add("assets/music.wav", trim_end=3)

linear_source = Spectrum2D(
    band_count=16,
    min_hz=80,
    max_hz=2400,
    sensitivity=350,
    attack_seconds=0.01,
    release_seconds=0.18,
    x=0.06,
    y=0.28,
    width=0.42,
    height=0.50,
    layout=Spectrum2DLinearLayout(anchor="bottom", band_mapping="forward"),
    colour="#9edbc7",
    bar_gap_ratio=0.25,
    min_bar_height_ratio=0.01,
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
    sensitivity=350,
    attack_seconds=0.01,
    release_seconds=0.18,
    x=0.61,
    y=0.28,
    width=0.28,
    height=0.50,
    layout=Spectrum2DRadialLayout(inner_radius_ratio=0.45),
    colour="#9edbc7",
    bar_gap_ratio=0.25,
    min_bar_height_ratio=0.01,
)
project.root.add(radial_source)
project.render("spectra.mp4", backend="cpu", overwrite=True)
```

The radial rectangle is approximately square in output pixels, despite its
unequal normalized width and height. `inner_radius_ratio=0.45` leaves an empty
centre; the default outward direction extends bars from that inner radius.
Set `start_angle_degrees` and `sweep_angle_degrees` for an arc, or choose
`direction="inward"` or `"both"` for a different growth direction.

## Adapt the response to a recording

| Setting | What to adjust |
| --- | --- |
| Frequency range | Focus on bass or treble, or widen it to show more of the mix. `max_hz` must fit prepared audio Nyquist. |
| `band_count` | More bands show finer frequency divisions but narrower bars; the current maximum is 48. |
| `sensitivity` | Increase it when bars barely move; lower it when many bars stay at full height. The example's 350 is tuned to these sparse generated tones. |
| Attack / release | Short attack follows onsets; longer release keeps bars legible between pulses. These are separate from scalar-signal `envelope`. |
| `min_bar_height_ratio` | Keep inactive bands visible as a baseline, or use zero to hide silent bars. |
| Layout and gaps | Change readability and geometry without changing the input recording. |

For a full-band recording, begin with fewer bands and a moderate sensitivity,
then tune at its quiet and loud sections. Do not assume the settings of this
sparse tone recording suit mastered music. The
[Spectrum2D reference](../../reference/sources/spectrum2d.md) lists defaults,
limits, gradients and presets.

## Reproduce this comparison

[Download audio_response.py](../../assets/examples/audio_response.py) and run
`python audio_response.py`. It generates its own three-second WAV and renders
this comparison as `audio-spectra.mp4`, plus the two
[scalar response studies](signals-and-audio-reactivity.md). Running it again
replaces those outputs.

All preview frames came from actual CPU renders. WGPU source dispatch exists,
but full visual parity for Spectrum2D is not verified; these previews provide
no additional WGPU evidence. Provenance, poster times, original checksums and
preview regeneration are recorded in the
[shared program credits](signals-and-audio-reactivity.md#credits-and-reproduction).
