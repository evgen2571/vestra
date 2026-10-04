# README feature showcase

[Watch / download the silent video](render.mp4)

![Animated feature showcase](preview.gif)

Ten seconds showing what the editor does, with simple feature labels:

- Moving layers with position, scale and rotation keyframes.
- Real video through two moving, feathered masks.
- An image resolving through saturation, blur and chromatic effects.
- Nested rings, grouped transforms, particles and an audio spectrum.

The MP4 and GIF are silent. Audio analysis uses the bundled original synth loop
to drive the spectrum, particles and reactive halos, with audio output disabled.
All inputs are bundled; no download is needed to render the checkout.
Footage is by Location Kenya under CC BY 3.0; see [asset credits](../ASSETS.md)
for the source/license links and redistribution requirements.

```bash
uv sync --locked --extra dev
uv run python examples/showcase/readme-demo/main.py
```

Output: `examples/output/readme-demo.mp4`, 1280×720, 30 fps, 10 seconds, CPU.
Use `--smoke` for the same timeline at 320×180 and 6 fps, or `--output PATH`.
`just showcase-refresh` regenerates committed videos, posters and animated previews.
