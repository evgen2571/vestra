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


Lift the quantization input with the existing tone response, independently of
palette animation and pattern scale:

```bash
uv run python examples/showcase/stylized-effects/main.py --look dither-blue-noise --palette ember --input-exposure 0.5 --input-gamma 1.5 --backend cpu --smoke
```

Tone controls are bindable in Python; the final amount keeps the original image
as its blend source. Their full-resolution preparation pass is opt-in.

Add fine detail before the same tone and palette processing:

```bash
uv run python examples/showcase/stylized-effects/main.py --look dither-blue-noise --palette ember --input-exposure 0.5 --input-gamma 1.5 --input-detail 1.5 --input-detail-radius 1 --backend cpu --smoke
```

For broader local contrast, use `--input-detail 0.5 --input-detail-radius 8`.
Try the same settings with `--palette mono` and `--palette ocean`. Detail adds
three full-resolution passes and defaults to zero; all controls remain editable
ordinary effect properties in Python.


## Fine-detail reference and ASCII diagnosis

[reference.py](reference.py) generates an original moving synthetic portrait
with facial features, thin hair bands, shaded cloth and a calm dark background.
It renders a four-second source/dither demonstration for each of the existing
monochrome, ember and ocean palettes, plus source-colored ASCII. All effects
are ordinary editable controls; no reference palette or image is embedded.

```bash
uv run python examples/showcase/stylized-effects/reference.py
VESTRA_WGPU_BACKEND=gl uv run python examples/showcase/stylized-effects/reference.py --backend wgpu
```

Add `--full-hd` for 1920×1080. Output is under
`target/stylized-reference/<width>-<backend>/`: lossless sampled frames,
canonical projects, source FFV1, short MP4s and `report.json`. The script checks
stationary frame equality, subject motion, repeated timestamps, visible tonal
structure and identical palette-index geometry across the three palettes.
Inspect native PNGs for one-pixel texture; reduced images and H.264 chroma
subsampling can soften it. This is a reproducible synthetic counterpart to the
reference's texture/tonal structure, not an exact reconstruction of different
photographic footage.

The dither example uses Bayer8, threshold `scale=1`, `input_scale=1`, stops
`(0, .22, .6, 1)`, input gamma `.9`, detail `1` and detail radius `1`.
Edit the palette colors freely while retaining dark-to-light order. Raise
`input_scale` with `input_filter="area"`, `"linear"` or `"nearest"` to compare
coarser input analysis independently of threshold scale; leave it at 1 for
fine source detail. Blue noise is available through the existing `matrix`
control when a less regular texture is wanted.

ASCII checks use zero source mixing with monochrome, source and palette colors.
Source coloring multiplies representative source color by sparse glyph coverage
against the authored background, so it can look much darker than its source.
Use existing `source_mix`, monochrome foreground, cell size/characters or an
ordinary preceding `ColorAdjust` for a brighter look. The script rejects
uniform/black output on its well-exposed source; a dark aesthetic alone does
not imply a failed renderer. Optional new brightness/density modes are deferred.
