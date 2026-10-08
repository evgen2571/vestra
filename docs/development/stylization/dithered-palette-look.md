# Detailed dithered-palette stylization

Status: **planned visual reference** for the Milestone 2 color/dithering work;
not an implemented feature or a dedicated hardcoded palette preset.
See the [active plan](../../plans/active/stylized-video-effects.md) and
[effect architecture](../architecture/effect-pipeline.md).

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

This is a *quality target* for the independently composable palette and
dithering effects in the current plan, not a new mandatory shader category.

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

## Proposed customization (names/defaults remain Milestone 1 decisions)

| Control | Intended behavior |
| --- | --- |
| Palette | User-defined ordered color stops/colors or compatible palette mapping; no fixed palette |
| Tonal/color mapping | Luminance-mapped palette levels and/or per-channel quantization, explicitly documented |
| Color resolution | Adjustable number/distribution of usable tone/color levels |
| Pattern mode | Ordered Bayer (for example 2x2, 4x4, 8x8); investigate optional blue noise |
| Detail level | Fine through coarse pattern/pixel scale, independently of palette selection |
| Dither strength | Control threshold spread and how much patterned midtone is retained |
| Tonal response | Tunable shadows, midtones and highlights, leveraging existing color/contrast effects wherever possible |
| Blending | Optional mixing with original footage or composition by ordinary effect semantics |
| Timing | Static deterministic pattern by default; if animated, use project time and an explicit loop period |
| Placement | Individual layer and global post-effects when semantically valid |

This table describes the **capability**, not a fixed Python constructor
signature. Let the implementation agent choose exact parameter types,
bounds, algorithms and naming consistently with the descriptor catalog.
Do not duplicate existing ColorAdjust/Contrast effects unnecessarily.

## Algorithm and renderer guidance

A candidate approach combines tonal adjustment, quantization with a
deterministic spatial threshold pattern, and palette mapping. Determine the
correct order of mapping/quantization/dither by testing fidelity rather
than assuming one ordering works in every color mode.

Investigate optional resolution scaling, nearest-neighbor appearance,
reliable pixel anchoring, luminance/color-space behavior, hard shadow
thresholds, anti-aliased edges, and retaining detail in textured areas.
Edge/local-contrast preservation is worth evaluating if naive quantization
loses recognizability, but it need not become a separate required effect.

Both CPU and WGPU must use consistent indexing, thresholds, alpha rules,
color math and border behavior. Prefer cheap and bounded shader resources,
cached static patterns, and no unnecessary full-frame intermediate buffers
or CPU/GPU readbacks per frame. Benchmark representative 1080p footage and
verify 4K device/resource behavior.

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

Once implemented, publish a reproducible example and update the
[effects guide](../../guides/python/effects.md),
[reference](../../reference/effects.md),
[support matrix](../../reference/feature-support.md) and, for footage or
other external assets, [license ledger](../../../examples/showcase/ASSETS.md).
