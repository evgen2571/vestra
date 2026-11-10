# Radial spectrum and audio-reactive motion

[Watch / download the rendered video](render.mp4)

![Animated preview of the Vestra render](preview.gif)

A twelve-second audio spectrum example with a master-audio RMS signal,
smoothed reactive geometry, seeded particles and bloom. Spectrum2D uses
prepared audio frequency analysis; the halo and particle opacity use scalar signals.

From the checkout after `uv sync --locked --extra dev`:

```bash
uv run python examples/showcase/audio-visualizer/main.py
```

Output: `examples/output/audio-visualizer.mp4`, 1280×720, 30 fps, 12 seconds, CPU.
Pass `--smoke` for the full timeline at 320×180 and 6 fps, or `--output PATH`.
The original synth bed is generated using Python's standard library; no download
is required. The waveform is bundled, so rendering does not run its generator.

The radial layout wraps 48 bands around a reactive halo, with a cyan-to-coral
gradient across the bands. The centred spectrum label and purple particle field
form a complete music-visualizer composition.

## Credits

`assets/audio/synth.wav` is an original sixteen-second synth loop by Vestra contributors,
dedicated to [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
[prepare-assets.py](../prepare-assets.py) generates 48 kHz, mono, 16-bit PCM with
Python's standard library; the generator code remains MIT licensed. This example
trims the first twelve seconds, applies gain and audio fades, and analyzes the
resulting mix. The README demo uses the same input for analysis but exports no audio.

Graphics and example code are MIT licensed. Headings use Manrope; see its
[copyright, accompanying OFL copy and source](../README.md#shared-font).
The video contains rendered text, not a redistributed font file.
