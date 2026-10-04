# README feature showcase

[Watch / download the silent looping video](render.mp4)

![Animated feature showcase](preview.gif)

Ten seconds showing what the editor does, with simple feature labels:

- Moving layers with position, scale and rotation keyframes.
- Real video revealed through an animated mask.
- Side-by-side grayscale and animated blur/colour effects on an image.
- Nested layers, a directional push and crossfades.
- An audio spectrum and geometry driven by master-audio signals.

The MP4 and GIF are silent. The final layout returns to the opening for a clean
loop. Audio analysis uses the bundled original synth loop to drive the spectrum
and geometry, with audio output disabled. All inputs are bundled; no download is
needed to render the checkout.
Footage is by Location Kenya under CC BY 3.0; see [asset credits](../ASSETS.md)
for the source/license links and redistribution requirements.

```bash
uv sync --locked --extra dev
uv run python examples/showcase/readme-demo/main.py
```

Output: `examples/output/readme-demo.mp4`, 1280×720, 30 fps, 10 seconds, CPU.
Use `--smoke` for the same timeline at 320×180 and 6 fps, or `--output PATH`.
`just showcase-refresh` regenerates committed videos, posters and animated previews.
