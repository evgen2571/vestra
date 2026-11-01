# Showcases

Six complete CPU-rendered compositions, with their Python source, rendered MP4s,
posters, silent animated previews and assets. Each project page displays a
GIF preview directly on GitHub; the MP4 link opens the full video file for download.

| Project | Render | Source / notes |
| --- | --- | --- |
| README demo | [Watch](readme-demo/render.mp4) | [10 seconds: motion design, masked footage and palette dithering](readme-demo/README.md) |
| README quickstart | [Watch](quickstart/render.mp4) | [3 seconds: asset-free animated cards](quickstart/README.md) |
| Effects lookbook | [Watch](lookbook/render.mp4) | [6 seconds: one photo, halftone and ASCII](lookbook/README.md) |
| Video editing | [Watch](real-media-edit/render.mp4) | [14 seconds: a silent three-shot night-city edit](real-media-edit/README.md) |
| Audio + spectrum | [Watch](audio-visualizer/render.mp4) | [12 seconds: spectrum, audio signals and particles](audio-visualizer/README.md) |
| Masks + groups | [Watch](compositing/render.mp4) | [8 seconds: photo cutouts, masks and grouped animation](compositing/README.md) |

The hero, lookbook and original three showcases are 1280×720 at 30 fps; the quickstart is
640×360 at 30 fps. All are rendered by Vestra itself. Only Audio + spectrum has an audio stream. Read [asset credits and licenses](ASSETS.md)
before redistributing inputs or outputs; third-party assets are not MIT licensed.

From a checkout with FFmpeg/ffprobe and the native package installed:

```bash
uv sync --locked --extra dev
just showcase-smoke                 # full timelines, offline test footage; small quickstart unchanged
just showcase                       # full renders from the bundled real inputs
just showcase-refresh               # regenerate committed MP4s, posters and GIFs
```

Full output goes to the ignored `examples/output/` directory. Smoke output goes
to `target/showcase-smoke/`. The smoke command deliberately substitutes generated
video fixtures in a temporary directory; it never overwrites the licensed inputs
or committed previews. Full rendering uses the real bundled footage.

`just showcase-assets` reproduces the source selections and synthesized audio.
It downloads a checksum-verified original into an ignored cache. A network
connection is only needed to reproduce the original footage, not to render the
checkout. The larger scenes accept `--smoke` and `--output PATH`; the small
quickstart uses fixed settings and writes to the current directory.

Inspect full videos before refreshing committed results. The renderer/probe
checks verify dimensions, frame counts, durations and audio streams; visual
quality and attribution still need human review. Codec bytes can vary with the
FFmpeg version even when the project and source inputs are unchanged.

The README quickstart is also checked against the code in the tutorial, so the
copy-paste example and its committed result share the same scene.

Full showcase MP4s use 1280×720 / 30 fps and the high quality tier (libx264
CRF 18 / medium); the small Python quickstart retains its 640×360 / 30 fps
balanced output. GIFs use 20 fps, 128 colours and a 480 px width (800 px for the
hero’s standalone project page). The media-edit GIF uses 400 px and 96 colours
to keep the moving footage reasonably small. It shows seconds 3.5–7.5, with
short fades at its ends; the linked MP4 is the complete
fourteen-second edit. GIF previews intentionally omit audio.
