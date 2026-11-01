# Make it move

[Watch / download the silent video](render.mp4) · [Python source](main.py)

![Vestra hero: animated cards, masked footage and palette dithering](preview.gif)

The README's ten-second hero is itself a Vestra composition, including all text:

- Three rounded cards rotate and move with eased keyframes.
- A night-city clip plays through a rounded mask with animated rotation.
- A photograph transitions into a four-colour Bayer-dithered look; its palette animates.

The title stays readable throughout, and a progress line follows the timeline.
All inputs are bundled; no download is needed. Footage is by Location Kenya
under CC BY 3.0; see [asset credits](../ASSETS.md) for source/license links and
redistribution requirements. This replaces the earlier feature-by-feature reel.

```bash
uv sync --locked --extra dev
uv run python examples/showcase/readme-demo/main.py
```

Output: `examples/output/readme-demo.mp4`, 1280×720, 30 fps, 10 seconds, CPU, no audio.
Use `--smoke` for the same timeline at 320×180 and 6 fps, or `--output PATH`.
`just showcase-refresh` regenerates committed videos, posters and animated previews.
