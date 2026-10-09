# Stylized effects

This reproducible scene synthesizes periodic moving footage and a four-second
original harmonic soundtrack with FFmpeg. It repeats both inputs twice, with matched
effect periods and continuous intensity keyframes. No media download is needed.

```bash
uv run python examples/showcase/stylized-effects/main.py --smoke --look ascii
VESTRA_WGPU_BACKEND=gl uv run python examples/showcase/stylized-effects/main.py --smoke --look crt --backend wgpu
```

Omit `--smoke` for 1920×1080 at 30 fps. Choose `ascii`, `custom-ascii`, `pseudo`,
`halftone`, `horizontal`, `vertical`, `crt`, `palette`, or `dither`. Composed
recipes are available as `analog-monitor`, `halftone-print`, and `sorted-neon`.
They attach ordinary editable effects in order. The custom
ASCII example uses a repository font and a short custom sequence; the ordinary
ASCII example uses the bundled font, palette animation, edges and source blending.
Generated assets, canonical JSON and renders stay under `target/stylized-showcase/`.

The source uses periodic analytic motion; repeated source frames are not added
at the seam. The bass pulse and three-note chord also complete an integer number of cycles.
Other footage, audio and authored animation must align separately to loop:
an effect's `period` does not make arbitrary footage seamless.

Character sets, sort directions and Bayer matrix choices are discrete. Animate
`amount` and source blending to fade a look using existing interpolation. The
[temporal diagnostics](../../../docs/development/stylization/temporal-diagnostics.md)
provide corresponding lossless CPU/WGPU frames, contact sheets and metrics.

Source generation and project code are original MIT-licensed work. The custom
font's license is recorded in [asset credits](../ASSETS.md).
