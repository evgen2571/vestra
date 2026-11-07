# Detailed dithered-palette stylization

This page records a visual design target and the regression criteria for
palette/dither changes. For current authoring, use the
[effects guide](../../guides/python/effects.md) and the
[reproducible example](#reproducible-reference-inspired-example). The target
below is not a claim that every palette/footage combination has been verified.

Use the [effect reference](../../reference/effects.md) for implemented controls
and the [effect architecture](../architecture/effect-pipeline.md) for resource
and renderer constraints.

## Reference and desired visual quality

Visual inspiration: [AcerolaFX example4.png](https://github.com/GarrettGunnell/AcerolaFX/blob/main/Examples/example4.png).

The reference exhibits clear silhouettes, high-contrast tonal masses, fine
ordered-looking pixel/stipple patterns in midtones and detailed local shading.
Large dark areas are calm; hair, fabric, contours and facial features retain
readable structure. Its visual identity comes from **pattern density, tonal
separation and fine detail**, **not** the screenshot's dark-red colors.
The precise source shader stack/settings cannot be inferred from the image.

**Design target:** Users must be able to reproduce this level of detail with
any suitable authored palette (monochrome, cool/warm, complementary, custom gradients,
neon and so on). Never hardcode a crimson palette or special-case hues.
This look is not inherently ASCII and needs no glyph atlas or custom font.

This quality target guides the independently composable palette and
dithering effects.

## Choosing controls

These controls are implemented; their exact combinations and limits belong in
the reference. Use this order when constructing a regression scene:

1. Use `OrderedDither` with authored palettes and tonal stops for fine structured
   texture. Bayer matrices provide regular patterns; seeded blue noise provides
   an irregular fixed pattern. `scale` controls threshold size, while `input_scale`
   and `input_filter` control analysis independently. Keep both scales at 1 to
   retain output-pixel detail.

2. Adjust entering-signal exposure/gamma and detail/radius before quantization to
   shape midtones and preserve local texture. Choose luminance, RGB, hue, Oklab or
   channel quantization deliberately: chromatic matching depends on palette colors,
   while tonal modes preserve index geometry when only the colors change and
   palette length, tonal stops and other controls stay fixed. Use
   `amount` for source blending and `phase`/`period` for repeatable palette motion.
   See the [reference](../../reference/effects.md)
   for ranges, rounding, alpha and mode compatibility.

3. Render the same scene with the three palette families below, inspect native
   frames and motion, then compare CPU and separately identified WGPU outputs.

Useful algorithm references:
[AcerolaFX Dither](https://github.com/GarrettGunnell/AcerolaFX/blob/main/Shaders/AcerolaFX_Dither.fx)
and [AcerolaFX PaletteSwap](https://github.com/GarrettGunnell/AcerolaFX/blob/main/Shaders/AcerolaFX_PaletteSwap.fx).
AcerolaFX uses the [MIT license](https://github.com/GarrettGunnell/AcerolaFX/blob/main/LICENSE.md).
If licensed code is copied or adapted, preserve its required notices;
the ReShade shader sources are not directly executable WGSL.

<span id="essential-visual-behavior"></span>
<span id="visual-and-technical-acceptance"></span>

## Regression checklist

Use the *same* deterministic scene with at least three substantially different
palettes: monochrome and two chromatic palettes. Choose a licensed or synthetic
source with fine texture, thin edges, highlights, deep shadows, smooth gradients
and recognizable foreground shapes.

- **Readable detail:** preserve silhouettes, contours, thin features, highlights
  and local midtone shading. Coarse posterization does not meet the fine-detail
  target even if structural tests pass.
- **Controlled texture:** dither density follows source tone, with calm dark
  negative space. Avoid unrelated speckling or smoothing away detail. Check
  fine patterns and deliberately coarse scales.
- **Palette independence:** when changing only colors in a tonal mode, keep
  palette length, stops and other controls fixed; index geometry and pattern
  density must stay the same. RGB, hue and Oklab matching intentionally depend
  on palette colors, so inspect their detail and contrast without requiring
  identical indices. Channel quantization ignores palettes.
- **Thresholds and extents:** test quantization ramps, boundary/tie cases,
  near-black/near-white values, small images, partial analysis cells and
  resolution/aspect-ratio changes.
- **Stable motion:** inspect moving footage and repeated/out-of-order timestamps
  for crawling, sparkle and frame-state dependence. Fixed grids stay anchored;
  intentional threshold steps and authored palette motion remain visible. Use
  [temporal diagnostics](temporal-diagnostics.md) for fixtures and seam checks.
- **Alpha and composition:** preserve transparent pixels and semitransparent
  edges through masks/mattes, source transforms and layer/global effect stacks.
- **Backend correctness:** compare actual CPU and WGPU frames using documented
  tolerances and [adapter evidence](../gpu-validation.md). Inspect full-resolution
  PNGs as well as motion; shader parsing and thumbnails are insufficient.
- **Cost and limits:** measure representative 1080p workloads and check 4K
  correctness/resources where supported. Record actual adapter and failures;
  [performance](../performance.md) separates measurement from correctness runs.

A passing result is a detailed, readable dithered image across suitable authored
palettes, rather than a noisy or low-detail image that only satisfies unit tests.

When changing these controls, update the
[effects guide](../../guides/python/effects.md),
[reference](../../reference/effects.md),
[support matrix](../../reference/feature-support.md) and, for footage or
other external assets, [example credits](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/stylized-effects/README.md).

## Reproducible reference-inspired example

Run [the synthetic portrait demonstration](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/stylized-effects/reference.py)
from that pinned revision, as described in [its guide](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/stylized-effects/README.md#fine-detail-reference-and-ascii-diagnosis).
Bayer8 at one output pixel, full-resolution entering input, nonuniform stops,
gamma .9 and radius-1 detail preserve fine cloth/hair bands, facial features
and calm shadows with three authored palettes. No new artistic mode was needed.
The generated contact images and short videos live under
`target/stylized-reference/`; inspect full-resolution PNGs as well as playback.
The original photographic scene is not reproduced; the quality comparison is
texture, tonal structure and recognizable source detail on generated footage.
See the [support matrix](../../reference/feature-support.md) for current limitations.
