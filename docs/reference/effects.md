# Effects

Visual effects are ordered canonical objects with `id`, `type` and descriptor parameters. Python classes in `vestra.effects` lower to them. The Rust descriptor catalog supplies canonical validation and generated-schema branches. No visual-effect constructor has an implicit parameter default unless the table says so: required values must be supplied. A scalar property accepts a number or `ScalarProperty` and may use keyframes/signal bindings; a plain track accepts keyframes but not signal bindings. `MotionTile.tile_center` and `RadialBlur.center` are dynamic point properties with a `base_value`, optional point keyframes, uniform `modifiers`, and independent `component_modifiers.x`/`.y` arrays.

Layer presentation uses two intrinsic effect stages:

```text
source sizing/crop → pre-transform effects → layer transform
→ post-transform effects → owned masks → Track Matte → opacity/blend
```

Motion Tile is the only pre-transform effect. Other effects execute after the
layer transform in declaration order. The public effect list remains one
ordered list; the compiler routes each canonical effect to its intrinsic
stage. Global post-effects remain post-composition.

The authored order is still the public order. For example, a canonical list
`[MotionTile, DirectionalBlur, ChromaticAberration]` samples the virtual tiles,
applies the layer transform, then runs Directional Blur followed by Chromatic
Aberration. Masks and track mattes consume that presented result.

| Python class / canonical type | Required parameters and limits | Scope |
| --- | --- | --- |
| `Brightness`, `Contrast`, `Saturation` | `amount`: finite scalar. | clip/global |
| `Tint` | `colour`, `amount`: 0..1 scalar. | clip/global |
| `GaussianBlur` | `radius`: 0..32 px scalar. | clip/global |
| `MotionTile` | `output_width_percent`, `output_height_percent`: 100..800% scalars; `tile_center`: dynamic normalized point property; `mirror_edges`: boolean. | clip only, pre-transform |
| `DirectionalBlur` | `radius`: 0..32 px scalar, `angle_degrees`: finite scalar. | clip/global |
| `ZoomBlur` | `radius`: 0..32 px scalar, `samples`: integer 2..32, `anchor`: normalized point, `direction=ZoomBlurDirection.CENTERED` (`centered`, `inward`, `outward`). | clip/global |
| `RadialBlur` | `amount`: 0..32 px scalar, `center`: dynamic normalized point property. | clip/global |
| `Glow` | `threshold`: 0..1, `radius`: 0..32 px, `intensity`: 0..4 scalars, `colour`. | clip/global |
| `Bloom` | `threshold`: 0..1, `radius`: 0..32 px, `intensity`: 0..4 scalars. | clip/global |
| `ChromaticAberration` | `amount`: 0..32 px scalar, `angle_degrees`: finite scalar. | clip/global |
| `Vignette` | `amount`: 0..1 scalar, `radius`: 0..2 scalar, `softness`: plain track in `(0, 2]`, `colour`. | clip/global |
| `Sharpen` | `amount`: 0..4 scalar, `radius`: 0..16 px scalar. | clip/global |
| `PaletteMap` / `palette_map` | `palette=("#000000", "#ffffff")`: 2..16 opaque colors; `mode="gradient"`; `amount=1`: 0..1 scalar; `phase=0`: finite scalar in cycles; `period=None`: optional positive seconds. | clip/global |
| `OrderedDither` / `ordered_dither` | Same palette, amount, phase and period; `mode="nearest"`; `strength=1`: 0..1 scalar; `matrix="bayer8"` (`bayer2`, `bayer4`, `bayer8`); `scale=1`: integer 1..32 output pixels per threshold cell. | clip/global |
| `ColorAdjust` | `exposure`: -8..8 scalar, `gamma`: `(0, 8]` scalar, `black_point`: plain track `[0, 1)`, `white_point`: plain track `(0, 1]`. | clip/global |
| `CameraShake` | `position_amount`, `rotation_degrees`, `scale_amount`: non-negative scalars; `frequency`: positive scalar; `seed`: unsigned integer; `attack`: non-negative seconds; `decay`: positive seconds; `active_interval=ActiveInterval()`. | clip only |
| `MotionBlur` | `intensity`: non-negative scalar, `shutter_angle`: 0..360 degrees scalar, `max_radius`: 0..32 px scalar, `samples`: integer 2..32. | clip only |

Numbers must be finite. Stack order is execution order within each intrinsic
stage. Motion Tile is valid only on a clip layer, only one Motion Tile may
appear in a layer stack, and it supports image, video, shape, text, and group
sources. Solid-color, Spectrum2D, and particle-system sources are rejected.
Motion Tile repeats the already-sized/cropped source over a virtual
sampling region while the final output remains the normal composition extent;
it does not allocate an expanded full-frame image. `mirror_edges=False` repeats
directly and `mirror_edges=True` alternates mirrored tiles, including negative
virtual coordinates. Sampling uses pixel-center coordinates (`n + 0.5`): for
each bilinear sample all four neighbors are mapped independently through
Euclidean tile division/remainder; mirrored odd tiles reverse the local offset
with `N - 1 - offset`. This makes repeat and mirror seams continuous for
negative and positive coordinates alike. Its RGBA samples are copied
together, so transparent source pixels remain transparent. CPU and WGPU use
the same coordinate model.
The canonical form uses each class's snake-case type such as `gaussian_blur`;
color values are canonical `#RRGGBB`/`#RRGGBBAA`.


Palette effects map encoded RGB luminance to uniformly spaced tonal stops.
Author colors from dark to light; the palette is not sorted automatically.
Colors may use `#RRGGBB` or opaque `#RRGGBBFF`; transparent palette colors are
rejected. The source alpha remains unchanged, including transparent pixels.
`amount=0` preserves the source and `amount=1` applies the full color mapping.

`PaletteMode.GRADIENT` interpolates adjacent palette stops and
`PaletteMode.NEAREST` chooses the nearest tonal stop. `PaletteMode.RAINBOW`
generates a 16-stop hue ramp with increasing brightness. Ordered Dither always
selects discrete tonal stops: its `gradient` and `nearest` modes use the same
ordered thresholds, while `rainbow` dithers the generated hue ramp. Changing
an authored palette of the same length leaves the threshold pattern unchanged.
`DitherMatrix.BAYER2`, `.BAYER4`, and `.BAYER8` choose its repeating ordered
matrix. `scale=1` preserves fine source detail; larger values enlarge the
pattern. `strength=0` gives ordinary nearest-stop quantization.

`phase` rotates colors smoothly in cycles without moving the spatial pattern.
When `period` is supplied, core adds `local_time / period` to the authored phase
and wraps the result. Layer effects use composition-local layer time; global
effects use composition time. The procedural color phase repeats after one
period when keyframes/signals are unchanged; source footage and audio do not
become loops. `amount`, `phase`, and dither `strength` accept keyframes and
signal bindings. Palette, mode, matrix, scale, and period are static controls.
Advanced authoring exposes `add_palette_map` and `add_ordered_dither` on clip
and post-effect collections, plus the generic descriptor-backed `add_effect`.

`Halftone` (`halftone`) offers `mode="luminance"`, `"source"`, or `"rgb"`;
`cell_size=6` (2–64 output pixels), `angle_degrees=15` (finite degrees),
`softness=0.5` (0–2 pixels), `foreground="#ffffff"`,
`background="#000000"` (opaque colors), `invert=False`, and `amount=1` (0–1).
Continuous numeric controls are bindable scalar properties. The rotated lattice
starts at the composition canvas origin, after the layer transform. RGB screens
have fixed 60-degree channel offsets. Each screen cell uses the alpha-weighted
mean of **all actual pixel centers** inside that rotated cell, including clipped
border cells; fully transparent RGB contributes nothing. One analysis pass
stores byte-quantized means in a bounded temporary; one resolve pass draws dots
with the original pixel alpha. No blur pass or frame history is used. Dot radius
is proportional to the square root of tone and reaches cell corners at white.
`softness=0` preserves intentionally hard print geometry; positive softness
smooths dot coverage analytically. Continuous cell-size/angle changes move the
lattice; membership changes remain stateless and discrete at pixel boundaries.

`PixelSort` (`pixel_sort`) has `direction="horizontal"` or `"vertical"`,
`order="ascending"` or `"descending"`, `lower_threshold=0.15` and
`upper_threshold=0.9` (bindable 0–1), `segment_length=64` (integer 2–256), and
bindable `amount=1` (0–1). Fixed blocks start at pixel zero on each row/column.
Within each block, contiguous eligible runs sort complete RGBA pixels by
`54*R + 183*G + 19*B`. Thresholds include their endpoints; alpha-zero and
ineligible pixels break runs. Equal integer luminance preserves original order
in either sort direction. Partial final blocks are bounded. Authored reversed
thresholds are invalid; crossing evaluated thresholds selects no pixels.
`amount` blends source and sorted pixels in premultiplied space. Sorting keeps
sharp edges intentionally; threshold crossings may create deliberate glitch
motion. The GPU sorts up to 256 pixels with 64 lanes and 7 KiB shared memory,
without a GPU readback or whole-frame staging allocation.

`Crt` (`crt`) defaults to `amount=1`, `curvature=0.08` (0–0.5),
`scanline_strength=0.2` (0–1), `scanline_spacing=2` (1–8 output pixels),
`mask_strength=0.15` (0–1), `mask_spacing=1` (integer 1–6 pixels),
`grain=0.025` (0–0.25), `jitter=0.35` (0–8 pixels), `flicker=0.025`
(0–0.25), `rolling_strength=0.06` (0–1), `rolling_width=0.12`
(0.01–1 of canvas height), `phase=0` (finite cycles), `period=None`
(optional positive finite seconds), and `seed=0` (unsigned 64-bit integer).
All numeric controls except mask spacing, period and seed are bindable.
Inverse-curvature sampling is alpha-aware; antialiased borders may become
transparent. Scanlines are pixel-footprint integrated and anchored at the first
output-row center. Phosphor masks are fixed output-pixel stripes. Grain/jitter
use fixed seeded sine/cosine coefficients, continuous owner-local time and no
frame counter. `period` repeats the procedural component, independently of the
source, audio or other animation. Modulo phase evaluation supports negative or
large finite phases without shader overflow. Intensity blends in premultiplied
space. Scanlines, masks and grain never create source alpha.

These effects support layers, groups and global post effects, with authored
order preserved. Static enum/color/integer settings change discretely; animate
`amount` for a source-to-look transition. Optional recipes in
`vestra.effects.recipes` return fresh ordinary effect chains rather than image
presets: `halftone_print()`, `analog_monitor(period=...)`, and
`sorted_neon(direction=...)`. Their individual effects remain editable.

### Cinematic ASCII and pseudo-ASCII

`Ascii` / `ascii` runs after the layer transform, or after composition as a
global effect. `PseudoAscii` is a Python convenience selecting
`glyph_style="geometric"`; it uses the same cell analysis, color and alpha
semantics with antialiased density-ranked dots, lines and crosses.

| Control | Python default | Contract |
| --- | --- | --- |
| `characters` | `" .:-=+*#%@"` | 1–256 independent Unicode scalars, authored dark-to-light order; duplicates retain intentional density weighting. `CHARACTER_SETS` provides `standard`, `dense` and `blocks` strings. |
| `edge_characters` | `"-\|/\\"` | Exactly four scalars: horizontal, vertical, slash, backslash. |
| `font` | `None` | Bundled DejaVu-derived regular face. Python accepts a font path; canonical JSON references a registered Font asset ID. |
| `cell_width`, `cell_height` | `8`, `12` | Bindable pixel sizes in 2..64 and 2..128, rounded once in core. |
| `mode` | `"hybrid"` | `fill`, `edges`, `hybrid`. |
| `glyph_style` | `"characters"` | `characters`, `geometric`. |
| `color_mode` | `"monochrome"` | `monochrome`, alpha-weighted cell `source`, `palette`, `rainbow`. |
| `foreground`, `background` | white, opaque black | Valid RGBA colors; transparent backgrounds are supported. |
| `palette` | black/white | 2–16 opaque authored colors; shared animated palette semantics. |
| `invert` | `False` | Reverse analyzed tone before density/color selection. |
| `edge_threshold`, `edge_strength` | `.15`, `1` | Bindable 0..1 and 0..4; strength saturates final coverage. |
| `source_mix`, `amount` | `0`, `1` | Bindable 0..1; mix original and styled output in premultiplied space. Either `amount=0` or `source_mix=1` preserves the input. |
| `phase`, `period` | `0`, `None` | Bindable finite phase in cycles; optional static positive finite period in owner-local seconds. Palette/rainbow motion repeats deterministically. |

The cell grid is anchored at the current composition canvas origin. Pixel
centers are `(x+.5, y+.5)`; transforms move footage through this fixed grid,
rather than moving the glyph lattice. Each cell visits its actual covered
pixels, including partial border cells, and ignores hidden RGB when alpha is
zero. RGB means and half-cell luminance means use alpha weighting. Those
region averages prefilter thin/noisy content without a separate full-frame
blur or a noisy single-pixel character decision. Entirely transparent cells
produce no visible support. The final styled alpha is intersected with each
original pixel's alpha.

Density selection crossfades neighboring character coverage only within a
narrow one-eighth interval around a density threshold. This is stateless:
random-access frame requests produce the same result. Edge selection uses
area-filtered horizontal/vertical luminance gradients, deterministic four-way
orientation bins and a smooth threshold band. Coverage stays antialiased;
source pixels and other intentionally crisp effects remain unchanged.

Glyphs are prepared once in fixed 32×48 tiles on 16 columns, with a common
baseline, and shared by CPU/WGPU. A five-level area-filtered coverage pyramid
reaches 2×3 tiles so small cells retain fine strokes. The largest atlas (256
fill and four edge glyphs) is 512×816, approximately 2.13 MiB including the
pyramid. Preparation validates source-pixel, per-asset and aggregate decoded
byte limits; WGPU validates atlas dimensions against the adapter before upload.
Cell-size/keyframe animation reuses these resources without font rebuilds or
GPU readbacks. Two packed RGBA8 cell metadata texels use an existing temporary
surface; resolve samples the retained original and prepared atlas.

Fonts use their first face, independent scalars, and no system fallback,
ligatures, grapheme, bidirectional or contextual shaping. Missing/control or
invisible non-space glyphs fail preparation with `VESTRA-ASCII-GLYPH` diagnostics.
The fixed glyph tile must contain the rasterized glyph; a font/glyph outside
that rasterization scope fails explicitly. Relative custom font paths resolve
against the project's base directory. The bundled font's complete notice is
in [DejaVuSans.txt](../../licenses/DejaVuSans.txt).

Enums, glyph strings/font and palette cardinality are discrete configurations.
Animate `amount` or `source_mix` for useful continuous transitions; no implicit
crossfade of incompatible font or character configurations is promised.
See the [ASCII JSON example](../../examples/effects/ascii.json),
[pseudo-ASCII example](../../examples/effects/pseudo-ascii.json), and
[looping showcase](../../examples/showcase/stylized-effects/README.md).
