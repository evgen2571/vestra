# Photo, halftone and ASCII

[Watch the MP4](render.mp4) · [Python source](main.py)

![A moving landscape shown as a photograph, halftone and ASCII](preview.gif)

One bundled CC0 landscape becomes three coordinated panels: the original
photograph, warm halftone and mint ASCII. Each panel uses the same eased zoom;
a rounded group mask keeps the crop consistent. All effects, masks, animation
and text are rendered by Vestra, with no postproduction compositing.

```bash
uv run python examples/showcase/lookbook/main.py
```

Output: `examples/output/lookbook.mp4`, 1280×720, 30 fps, six seconds, CPU, silent.
Use `--smoke` for 320×180 at 6 fps or `--output PATH` for another destination.
`just showcase-refresh` regenerates the committed MP4, poster and GIF.

The photo is by Dimitry Anikin (CC0); headings use Manrope (OFL). ASCII uses
Vestra's bundled DejaVu face. See [asset and font credits](../ASSETS.md) and
[software notices](../../../THIRD_PARTY_NOTICES.md). Source code and original
layout are MIT licensed.
