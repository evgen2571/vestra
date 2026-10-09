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

Compare fine Bayer and blue noise over the same generated footage with
`--look dither` and `--look dither-blue-noise`. Use `--palette mono`, `ember`,
or `ocean` for three editable color choices; the pattern is independent of
palette hue. For example:

```bash
uv run python examples/showcase/stylized-effects/main.py --smoke --look dither-blue-noise --palette ember
```

Both patterns use one output pixel per threshold cell. Blue noise uses a
stationary seeded 32×32 tile (`seed=37`); only the palette phase animates.

For source-color matching, compare `--mode nearest_rgb` and `--mode nearest_hue`
on either dither look. The RGB mode measures encoded channel distance; the hue
mode weighs circular hue by saturation and includes saturation and brightness.
Both accept arbitrary palette order and mix the two closest entries spatially.
The default `--mode nearest` retains luminance-indexed tonal mapping.

`--mode rgb_channels --levels 4` quantizes each channel to four levels (64 RGB
colors). Try 2 or 8 for coarse or finer detail. This mode uses a color cube and
ignores the authored palette and its phase. All channels share the same rank
pattern; 256 levels preserve the source bytes even at full dither strength.

`--mode nearest_oklab` uses deterministic fixed-point perceptual matching in
Oklab, interpreting source bytes as sRGB. Compare it with `nearest_rgb` using the
same `--palette`: matching considers lightness and chroma rather than encoded
channel distance. Exact palette colors are preserved. Phase still rotates the
ordinary editable palette; no additional color preset is hidden in this mode.

Use uneven tonal positions to allocate more palette colors to shadows:

```bash
uv run python examples/showcase/stylized-effects/main.py --look dither-blue-noise --palette ember --stops 0 0.18 0.55 1 --backend cpu --smoke
```

`--stops` requires one increasing position per palette color and tonal
`--mode nearest` (the default); chromatic/channel modes reject this control.

Add `--interpolation oklab` to use perceptual interpolation for the animated
palette colors, retaining discrete dither output. The `palette` look also uses
this control for gradient segments. The default `rgb` retains the original
encoded RGB interpolation. Output gamut handling is documented in the
[effect reference](../../../docs/reference/effects.md).
