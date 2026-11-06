---
description: An audio visualization combining frequency analysis, smoothed loudness-driven motion and deterministic particles.
---

# Audio-reactive graphics

This example uses two measurements of a synthesized soundtrack. Frequency-band
analysis controls the spectrum bars. Overall loudness controls the halo's size
and the particle layer's opacity.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/audio-reactive.webp" aria-label="Twelve-second spectrum animation with synthesized music">
    <source src="../assets/gallery/audio-reactive.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/audio-reactive.mp4">Download the preview</a>.
  </video>
  <figcaption>Forty spectrum bars mirror above and below a baseline. A circular halo pulses gently behind them while small particles drift upwards. The preview includes synthesized music and no speech.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/audio-visualizer/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/audio-visualizer/render.mp4)

## Reproduce the example

After the [one-time gallery setup](index.md#run-the-examples), run:

```bash
uv run --no-sync python examples/showcase/audio-visualizer/main.py
```

Open `examples/output/audio-visualizer.mp4`: twelve seconds, 1280 × 720, 30 fps,
with audio. The script replaces this output on each run.
Use `--output PATH` for another destination or `--smoke` for a smaller version
of the full timeline.

The synthesized soundtrack and Manrope font are bundled. If you want to
regenerate the original soundtrack, the asset generator uses Python's standard
library and needs no network download:

```bash
uv run --no-sync python examples/showcase/prepare-assets.py --audio-only
```

## Connect audio measurements to properties

The spectrum source and property bindings have separate roles:

| Picture | Measurement | What changes |
| --- | --- | --- |
| Spectrum bars | Frequency-band analysis through `Spectrum2D` | The height of each bar |
| Halo | Master-audio RMS signal | The circle's scale |
| Particles | The same RMS signal | The particle layer's opacity |

The code remaps loudness into a narrow scale range, clamps it and adds an
attack/release envelope. This smooths sudden changes in the halo's size while
preserving its response to the soundtrack. The particle system uses a fixed
seed so the layout is repeatable.

Start with [Audio and signals](../concepts/audio-and-signals.md), then see
[Audio-reactive authoring](../guides/python/signals-and-audio-reactivity.md) and
[The Spectrum2D source](../reference/sources/spectrum2d.md).

## Credits

The original synth loop is dedicated to
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) by Vestra contributors.
The graphics are original project work; the Manrope font is SIL OFL 1.1.
See the [preview provenance](../assets/gallery/README.md).

[Back to the gallery](index.md)
