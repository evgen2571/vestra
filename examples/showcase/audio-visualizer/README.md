# Radial spectrum and audio-reactive motion

[Full video with sound](render.mp4) · [Python source](main.py)

![A circular audio spectrum with an animated halo and particles](preview.gif)

Frequency bands draw the spectrum. The overall level of the synth track
drives the halo and particle opacity.

## Render

Run this from a [development checkout](../../../CONTRIBUTING.md#2-set-up-a-checkout).

```bash
uv run python examples/showcase/audio-visualizer/main.py
```

Writes `examples/output/audio-visualizer.mp4` (12 seconds). Add `--smoke` for a
quick preview or `--output PATH` to choose the destination.

## Credits

Vestra contributors dedicated the bundled `assets/audio/synth.wav` to
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/). The scene trims
the sixteen-second loop, applies gain and fades, and analyzes the result.
[MIT-licensed synth generator](../prepare-assets.py) · [Manrope font credits](../README.md#shared-font)
