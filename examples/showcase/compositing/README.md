# Photo cutouts with masks and groups

[Watch / download the rendered video](render.mp4)

![Animated preview of the Vestra render](preview.gif)

An eight-second example of one photograph composited through three geometric
masks: a circle, a rounded rectangle and a rectangle. Each nested group moves
as a unit while its image zooms independently; different saturation settings
make the treatments easy to compare. The output is silent.

From the checkout after `uv sync --locked --extra dev`:

```bash
uv run python examples/showcase/compositing/main.py
```

Output: `examples/output/compositing.mp4`, 1280×720, 30 fps, 8 seconds, CPU.
Pass `--smoke` for the full timeline at 320×180 and 6 fps, or `--output PATH`.
The OFL font and CC0 photograph are bundled; no downloads are needed.
See the sources below and the [shared Manrope notice](../README.md#shared-font).

## Photo source

See the [shared landscape record](../../../assets/images/README.md) for the
source, license, original checksum and reproduction command. This scene crops,
zooms, masks and adjusts saturation; the [lookbook](../lookbook/README.md) also
applies effects. Graphics and example code are MIT licensed.
The [Manrope license](../../../assets/fonts/Manrope-OFL.txt) stays beside the font.
