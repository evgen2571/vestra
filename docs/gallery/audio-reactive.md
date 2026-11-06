---
description: An audio visualization with a radial frequency spectrum, smoothed loudness-driven motion and deterministic particles.
---

# Audio-reactive graphics

This example uses two measurements of a synthesized soundtrack. Frequency-band
analysis controls 48 bars arranged around a circle. Overall loudness controls
the ring's size and the particle layer's opacity.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/audio-reactive.webp" aria-label="Twelve-second radial spectrum animation with synthesized music">
    <source src="../assets/gallery/audio-reactive.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/audio-reactive.mp4">Download the preview</a>.
  </video>
  <figcaption>Cyan-to-coral bars extend out from a ring, with particles drifting behind them. The ring expands and contracts with the soundtrack. The preview includes synthesized music and no speech.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/audio-visualizer/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/audio-visualizer/render.mp4)

## Reproduce the example

After the [one-time gallery setup](index.md#run-the-examples), run:

```bash
uv run --no-sync python examples/showcase/audio-visualizer/main.py
```

Open `examples/output/audio-visualizer.mp4`: twelve seconds, 1280 × 720, 30 fps,
with audio. The script replaces this output on each run.
Use `--output PATH` for another destination or `--smoke` for a smaller version
of the full timeline.

The synthesized soundtrack and Manrope font are bundled. To regenerate the
original soundtrack without a network download:

```bash
uv run --no-sync python examples/showcase/prepare-assets.py --audio-only
```

## Connect audio measurements to properties

The spectrum source and property bindings have separate roles:

| Picture | Measurement | What changes |
| --- | --- | --- |
| Spectrum bars | Frequency-band analysis through `Spectrum2D` | The length of each bar |
| Ring | Master-audio RMS signal | The circle's scale |
| Particles | The same RMS signal | The particle layer's opacity |

`Spectrum2DRadialLayout` wraps the bands around the centre, and a gradient
assigns colours across the bands. A bloom effect softens the brightest parts.

The RMS binding remaps loudness into a narrow scale range, clamps it and adds
an attack/release envelope. This smooths sudden changes in the ring's size
while preserving its response to the soundtrack. The particle system uses a
fixed seed so the layout is repeatable.

Start with [Audio and signals](../concepts/audio-and-signals.md), then see
[Audio-reactive authoring](../guides/python/signals-and-audio-reactivity.md) and
[The Spectrum2D source](../reference/sources/spectrum2d.md).

## Credits

The original synth loop is dedicated to
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) by Vestra contributors.
The graphics are original project work; the Manrope font is SIL OFL 1.1.
See the [synth dedication and font notice](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/audio-visualizer/README.md#credits).

## Preview details

This is a downscaled copy of the complete CPU render: 640 × 360, 30 fps,
H.264/yuv420p with MP4 fast start. The WebP poster is the frame at 3 seconds.
The original render and cleaned source are together at the
[reviewed example revision](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/audio-visualizer/README.md).
The source render's SHA-256 is `2abfd5976d5c948cc77aaec688fd73f6bd9044940ecc8b5718a313f9d97f4db5`.

[Reproduce the delivery copies](../development/documentation.md#reproduce-gallery-previews).
The previews contain rendered text; they do not distribute font files.

[Back to the gallery](index.md)
