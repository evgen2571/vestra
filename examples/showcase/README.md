# Showcases

Four complete CPU-rendered compositions, with their Python source, rendered MP4s,
posters, silent animated previews and assets. Each project page displays its
GIF directly on GitHub; the MP4 link opens the full video file for download.

| Project | Render | Source / notes |
| --- | --- | --- |
| README demo | [Watch](readme-demo/render.mp4) | [10 seconds: animated layers, media, masks, effects, transitions and audio](readme-demo/README.md) |
| Video editing | [Watch](real-media-edit/render.mp4) | [14 seconds: a three-shot night-city edit with audio](real-media-edit/README.md) |
| Audio + spectrum | [Watch](audio-visualizer/render.mp4) | [12 seconds: spectrum, audio signals and particles](audio-visualizer/README.md) |
| Masks + groups | [Watch](compositing/render.mp4) | [8 seconds: nested geometry, masks and animated matte](compositing/README.md) |

All committed videos are 1280×720 at 30 fps, rendered by Vestra itself. The README demo, Video editing and Audio + spectrum have audio. Read [asset credits and licenses](ASSETS.md)
before redistributing inputs or outputs; third-party assets are not MIT licensed.

From a checkout with FFmpeg/ffprobe and the native package installed:

```bash
uv sync --locked --extra dev
just showcase-smoke                 # full timelines, 320×180 / 6 fps, offline test footage
just showcase                       # full renders from the bundled real inputs
just showcase-refresh               # regenerate committed MP4s, posters and README GIF
```

Full output goes to the ignored `examples/output/` directory. Smoke output goes
to `target/showcase-smoke/`. The smoke command deliberately substitutes generated
video fixtures in a temporary directory; it never overwrites the licensed inputs
or committed previews. Full rendering uses the real bundled footage.

`just showcase-assets` reproduces the source selections and synthesized audio.
It downloads a checksum-verified original into an ignored cache. A network
connection is only needed to reproduce the original footage, not to render the
checkout. Individual `main.py` files also accept `--smoke` and `--output PATH`.

Inspect full videos before refreshing committed results. The renderer/probe
checks verify dimensions, frame counts, durations and audio streams; visual
quality and attribution still need human review. Codec bytes can vary with the
FFmpeg version even when the project and source inputs are unchanged.
