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
| `OrderedDither` / `ordered_dither` | Same palette, amount, phase and period; `mode="nearest"`; `strength=1`: 0..1 scalar; `matrix="bayer8"` (`bayer2`, `bayer4`, `bayer8`, `blue_noise`); `seed=0`: integer 0..4294967295; `scale=1`: integer 1..32 output pixels per threshold cell. | clip/global |
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


By default, tonal palette modes map encoded RGB luminance to uniformly spaced stops.
For those modes author colors from dark to light; the palette is not sorted automatically.
Colors may use `#RRGGBB` or opaque `#RRGGBBFF`; transparent palette colors are
rejected. The source alpha remains unchanged, including transparent pixels.
`amount=0` preserves the source and `amount=1` applies the full color mapping.

`stops=None` preserves the original uniform mapping byte for byte. For `gradient`
or `nearest`, supply one static position per authored color, for example
`palette=("#000000", "#ff0000", "#ffffff"), stops=(0, 0.25, 1)`.
Positions must increase, start at zero and end at one. They are rounded half up
onto the encoded-luminance grid `0..65280`; positions that collapse to the same
grid value are rejected. Palette order is retained. Phase/period animate colors
through these fixed positions without moving or sorting the positions.
Custom stops are rejected for rainbow, chromatic matching and channel modes,
whose palettes do not represent authored tonal intervals.

For custom stops, the source key is `54*r + 183*g + 19*b`. Locate its surrounding
stop keys; gradient mapping interpolates encoded RGB bytes in that interval,
rounding half up. Nearest mapping chooses the upper color at an exact midpoint.
Dither rounds the local interval fraction half up to `1/4096`, blends it with
the nearest choice's probability using strength rounded to `1/4096`, and rounds
again. It chooses the upper color when the existing threshold is at least
`1-probability`. Thus zero strength gives nearest mapping and full strength
preserves the local tonal fraction spatially. Endpoints retain authored bytes;
source alpha remains unchanged.

`PaletteInterpolation.RGB` (`interpolation="rgb"`, the default) retains encoded
RGB interpolation and the original phase arithmetic. `PaletteInterpolation.OKLAB`
(`"oklab"`) interpolates in the same deterministic Q10 Oklab space used for
perceptual matching. It applies to Palette Map's gradient segments (uniform or
custom stops) and to authored palette color motion in both effects. Dither still
selects discrete evaluated palette colors; nearest modes use interpolation only
for palette motion. Rainbow retains its generated HSV phase behavior, while
Palette Map can interpolate its generated segments in Oklab. Channel quantization
ignores interpolation along with palette/phase.

Oklab interpolation rounds weighted biased Q10 coordinates half up. The inverse
Lab-to-LMS-root matrix uses Q15 coefficients; signed matrix rounding is half away
from zero. Roots use a conservative signed `±1100/1024` arithmetic bound, then
cube to Q16 LMS with the same signed rounding. The inverse linear RGB matrix uses
Q12 coefficients with normalized row sums. Linear RGB channels are clipped to
`0..1`; each channel selects the nearest entry in the shared 256-entry sRGB decode
table, resolving encoding ties upward. This is channel gamut clipping rather
than chroma-preserving gamut compression. Exact segment endpoints and identical
neighbor colors return authored bytes directly. Phase fractions round to
`1/65280` before Oklab mixing; RGB phase evaluation keeps its original arithmetic.
Source alpha and blend amount retain their existing semantics. Integer conversion
can introduce small rounding steps; interpolation adds no passes or textures.

`PaletteMode.GRADIENT` interpolates adjacent palette stops and
`PaletteMode.NEAREST` chooses the nearest tonal stop. `PaletteMode.RAINBOW`
generates a 16-stop hue ramp with increasing brightness. Ordered Dither always
selects discrete tonal stops: its `gradient` and `nearest` modes use the same
ordered thresholds, while `rainbow` dithers the generated hue ramp. Changing
an authored palette of the same length leaves the threshold pattern unchanged.
`DitherMatrix.BAYER2`, `.BAYER4`, and `.BAYER8` choose its repeating ordered
matrix. `scale=1` preserves fine source detail; larger values enlarge the
pattern. `DitherMatrix.BLUE_NOISE` selects an original, toroidally seamless
32×32 void-and-cluster rank tile with 1024 distinct thresholds. It stays fixed
in output pixels, independent of source motion, frame order or palette colors.
`seed=0` uses the original orientation. First fold the unsigned seed with
`seed ^ (seed >> 13) ^ (seed >> 26)`; bits 0..4 and 5..9 shift the x/y origin,
bit 10 swaps axes, and bits 11/12 reflect x/y. All authored bits participate in the fold. Seeds can alias; they select tile transforms,
not independent random textures. Bayer ignores `seed`. `scale` enlarges either
pattern, independently of source analysis. Neither mode allocates frame-sized
pattern resources or reads frames back to the CPU. The tile can be reproduced
with `uv run --no-project python scripts/generate-blue-noise.py`; the generator
checks threshold coverage and low-frequency suppression at five tone cutoffs.
`strength=0` gives ordinary nearest-stop quantization.

`PaletteMode.NEAREST_RGB` (`nearest_rgb`) selects the authored color with the
smallest squared distance in encoded RGB byte space. `PaletteMode.NEAREST_HUE`
(`nearest_hue`) uses integer HSV distance. These modes accept arbitrary color
order, preserve exact palette colors, and resolve equal distances to the first
authored entry. Neither mode performs linear-light or perceptual conversion.
HSV value is the maximum channel, saturation is `floor(255 * (max-min) / max)`
(zero for gray), and hue uses a wrapped 1530-step circle. Hue sectors are
`255*(g-b)/range`, `510+255*(b-r)/range`, or `1020+255*(r-g)/range` for a maximum
red, green, or blue channel respectively; signed division truncates toward zero.
The hue-distance component is the shortest wrapped difference divided by three,
then multiplied by the smaller saturation and divided by 255, with each division
truncated. Squared hue, saturation and value differences are summed. Hue therefore
has no influence when either color is gray. Colors stay within byte gamut.

In these two chromatic modes, Ordered Dither chooses the two nearest palette
entries. If their squared distances are `d0 <= d1`, the second entry's probability
is `d0 / (d0+d1)`, or zero when both distances are zero. Probability and strength
are rounded half up to 1/4096, multiplied and rounded to the same precision,
then compared with the existing matrix threshold. At zero strength it always
chooses the nearest entry; increasing strength blends the pair spatially.
Palette Map always chooses the nearest entry. Source alpha and transparent
pixels retain the same policy as the legacy modes.

`PaletteMode.NEAREST_OKLAB` (`nearest_oklab`) matches an arbitrary authored
palette by squared Euclidean distance in a deterministic integer approximation
of [Oklab](https://bottosson.github.io/posts/oklab/). Encoded RGB input is interpreted
as sRGB/D65: decode each byte with the standard sRGB transfer function to a
Q16 linear value, rounded half up. The 256-entry table is generated at 60-decimal
precision. Oklab's linear-sRGB matrices are rounded to Q15; each row's largest
absolute coefficient is adjusted to preserve its sum (one for LMS/lightness,
zero for chroma). LMS sums round half up to Q16, cube roots round to Q10 using
integer binary search, and the final Lab matrix rounds ties away from zero.
Lab coordinates have 1/1024 resolution; a/b use a storage bias of 512, which
cancels in distance calculations. No inverse conversion or gamut mapping is
needed: output always selects authored opaque palette bytes and preserves
source alpha. The authored order is retained.

An exact RGB palette match takes precedence over feature-distance ties, so
nearby colors that share rounded Lab coordinates still preserve their exact
authored bytes. Other ties choose the first entry. Ordered Dither uses the same
two-nearest, Q12 inverse-squared-distance probability as the RGB/hue modes;
64-bit CPU arithmetic and bounded WGSL long division avoid intermediate
overflow. This is a documented fixed-point approximation, not a full-precision
color-management conversion. Reproduce its shared Rust/WGSL constants with
`uv run --no-project python scripts/generate-oklab.py`.

`PaletteMode.RGB_CHANNELS` (`rgb_channels`) quantizes encoded R, G and B
independently to `levels=4` uniformly spaced values per channel. `levels` is a
static integer from 2 through 256; it is ignored by the other modes. Palette,
phase and period do not affect channel quantization, but still obey their normal
validation rules. At 256 levels the full-strength effect preserves source RGB
exactly. Source alpha and transparent pixels remain unchanged.

For channel byte `c` and `n=levels-1`, compute `position=c*n`, integer
`lower=position/255` and `fraction=position%255`. Palette Map chooses the nearest
index, rounding up when `fraction>=128`. Ordered Dither rounds `fraction/255`
half up to 1/4096 and blends that probability with the nearest index's probability
(zero or one), using dither strength rounded to 1/4096. A threshold below that
probability chooses the upper index; otherwise it chooses the lower. The final
channel byte is `round_half_up(255*index/n)`. All three channels share the same
spatial rank. This preserves average channel tone at full dither strength and
provides ordinary nearest-channel quantization at zero strength.

`phase` rotates colors smoothly in cycles without moving the spatial pattern.
When `period` is supplied, core adds `local_time / period` to the authored phase
and wraps the result. Layer effects use composition-local layer time; global
effects use composition time. The procedural color phase repeats after one
period when keyframes/signals are unchanged; source footage and audio do not
become loops. `amount`, `phase`, and dither `strength` accept keyframes and
signal bindings. Palette, mode, matrix, scale, seed, and period are static controls.
`levels` is also a static control. Advanced authoring exposes `add_palette_map` and `add_ordered_dither` on clip
and post-effect collections, plus the generic descriptor-backed `add_effect`.

`Halftone` (`halftone`) offers `mode="luminance"`, `"source"`, or `"rgb"`;
`cell_size=6` (2–64 output pixels), `angle_degrees=15` (finite degrees),
`softness=0.5` (0–2 pixels), `foreground="#ffffff"`,
`background="#000000"` (opaque colors), `invert=False`, and `amount=1` (0–1).
Continuous numeric controls are bindable scalar properties. The rotated lattice
starts at the composition canvas origin, after the layer transform. RGB screens
have fixed 60-degree channel offsets. Each screen cell uses the alpha-weighted
mean of **all actual pixel centers** inside that rotated cell, including clipped
border cells; fully transparent RGB contributes nothing. Cell membership uses
shared Q16 lattice coefficients and integer pixel-center coordinates, preventing
GPU fused floating-point arithmetic from changing diagonal boundary ties. Cell
size and direction coefficients use 16 fractional bits; antialiased dots remain
floating-point geometry. One analysis pass
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
Inverse-curvature sampling is alpha-aware. CPU and WGPU use shared Q15
normalized coordinates and curvature, then a 1/256-pixel sampling lattice.
Zero curvature preserves exact pixel centers; jitter coefficients are prepared
once per frame. Integer arithmetic stays bounded at the 8192-pixel canvas limit. Antialiased borders may become transparent; pixels whose final byte
alpha is zero have zero RGB. Scanlines are pixel-footprint integrated and
anchored at the first output-row center. Phosphor masks are fixed output-pixel
stripes. Grain/jitter
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

Cell dimensions evaluate to whole pixels, rounded to the nearest integer
before either renderer runs. Keyframes and signals therefore resize the grid
in discrete one-pixel steps; cell sizing is not a continuously interpolated
geometry control. Use `amount` or `source_mix` fades around deliberate grid
changes when a smooth transition is desired.

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
against the project's base directory. High-level Python registers and shares
font assets across clip/global effects, transition presentations and owned
mask Group child effects. Canonical transition `font` fields use Font asset
IDs; missing or wrong-kind assets report the effect's `/font` validation path.
The bundled font's complete notice is
in [DejaVuSans.txt](../../licenses/DejaVuSans.txt).

Enums, glyph strings/font and palette cardinality are discrete configurations.
Animate `amount` or `source_mix` for useful continuous transitions; no implicit
crossfade of incompatible font or character configurations is promised.
See the [ASCII JSON example](../../examples/effects/ascii.json),
[pseudo-ASCII example](../../examples/effects/pseudo-ascii.json), and
[looping showcase](../../examples/showcase/stylized-effects/README.md).
