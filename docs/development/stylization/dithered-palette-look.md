# Detailed dithered-palette stylization

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

**Requirement:** Users must reproduce this level of detail with any suitable
authored palette (monochrome, cool/warm, complementary, custom gradients,
neon and so on). Never hardcode a crimson palette or special-case hues.
This look is not inherently ASCII and needs no glyph atlas or custom font.

This quality target guides the independently composable palette and
dithering effects.

## Essential visual behavior

- Preserve recognizable contours, thin features, subject silhouettes,
  highlights and nuanced midtone textures rather than flattening to a few
  broad posterized shapes.
- Produce structured fine-grained dither clusters; offer both subtle/fine
  detailing and deliberately coarse pixel art via adjustable scale.
- Preserve dark negative space and controlled shadow texture instead of
  filling the entire image with random noise.
- Map tonal transitions to palette levels via dithering; avoid arbitrary
  speckling unrelated to source tone and avoid over-smoothing away texture.
- Make spatial character independent of palette choice: changing colors
  should not unexpectedly change dot density or edge geometry.
- Keep pixel/grid positioning deterministic and stable across frames by
  default. Do not let moving footage exhibit avoidable temporal crawling,
  sparkle or output dependent on evaluation order.
- Respect transparent pixels, alpha edges, masks, mattes, source transforms
  and layer/global effect stacking.

## Choosing controls

Use `OrderedDither` with authored palettes and tonal stops for fine structured
texture. Bayer matrices provide regular patterns; seeded blue noise provides
an irregular fixed pattern. `scale` controls threshold size, while `input_scale`
and `input_filter` control analysis independently. Keep both scales at 1 to
retain output-pixel detail.

Adjust entering-signal exposure/gamma and detail/radius before quantization to
shape midtones and preserve local texture. Choose luminance, RGB, hue, Oklab or
channel quantization deliberately: chromatic matching depends on palette colors,
while tonal modes preserve index geometry across equal-length palettes. Use
`amount` for source blending and `phase`/`period` for repeatable palette motion.
See the [reference](../../reference/effects.md)
for ranges, rounding, alpha and mode compatibility.

Useful algorithm references:
[AcerolaFX Dither](https://github.com/GarrettGunnell/AcerolaFX/blob/main/Shaders/AcerolaFX_Dither.fx)
and [AcerolaFX PaletteSwap](https://github.com/GarrettGunnell/AcerolaFX/blob/main/Shaders/AcerolaFX_PaletteSwap.fx).
AcerolaFX uses the [MIT license](https://github.com/GarrettGunnell/AcerolaFX/blob/main/LICENSE.md).
If licensed code is copied or adapted, preserve its required notices;
the ReShade shader sources are not directly executable WGSL.

## Visual and technical acceptance

Verify **at least three substantially different palettes**, including a
monochrome palette and two different chromatic palettes, over the *same*
deterministic source scene. The same spatial details must remain legible.
Use a licensed or synthetic fixture that includes high-frequency detail,
thin edges, highlights, deep shadows, smooth gradients and recognizable
foreground shapes.

Additional checks:

- Quantization ramps, threshold boundaries, near-black/near-white values,
  fine pattern scales, resolution/aspect-ratio changes and small images.
- Moving video with repeated/out-of-order timestamps to expose crawling,
  flickering and accidental frame-state dependence.
- Transparent/semitransparent edges, masks/mattes, transformations and
  composition with other effects.
- Real CPU and WGPU output compared with documented tolerances; shader
  parsing alone is insufficient.
- Performance/resource measurements at 1080p and 4K correctness where
  supported; record actual adapter and failure diagnostics.

**Acceptance is visual, not merely structural:** a finely detailed,
readable dithered image across arbitrary palettes. A coarse posterization
or noisy low-detail image should not pass just because unit tests pass.

When changing these controls, update the
[effects guide](../../guides/python/effects.md),
[reference](../../reference/effects.md),
[support matrix](../../reference/feature-support.md) and, for footage or
other external assets, [license ledger](../../../examples/showcase/ASSETS.md).


## Reproducible reference-inspired example

Run [the synthetic portrait demonstration](../../../examples/showcase/stylized-effects/reference.py)
as described in [its guide](../../../examples/showcase/stylized-effects/README.md#fine-detail-reference-and-ascii-diagnosis).
Bayer8 at one output pixel, full-resolution entering input, nonuniform stops,
gamma .9 and radius-1 detail preserve fine cloth/hair bands, facial features
and calm shadows with three authored palettes. No new artistic mode was needed.
The generated contact images and short videos live under
`target/stylized-reference/`; inspect full-resolution PNGs as well as playback.
The original photographic scene is not reproduced; the quality comparison is
texture, tonal structure and recognizable source detail on generated footage.
See the [support matrix](../../reference/feature-support.md) for current limitations.
