# Effects

Use the [visual catalog](../guides/python/effect-catalog.md) to choose an effect,
the [basic guide](../guides/python/effects.md) to attach one, and the generated
[parameter reference](effect-parameters.md) for defaults, bounds, units, animation
support, stage and scope. This page specifies the contracts shared by those controls.

## Attachment and canonical form

Effects are ordered canonical objects with an `id`, a snake-case `type` and
parameters. Python classes in `vestra.effects` lower to these objects; the Rust
descriptor catalog owns validation and generated schema branches. Python defaults
and omitted JSON fields are separate contracts.

There are 23 authored canonical effects and 24 high-level constructors:
`PseudoAscii` adapts `Ascii` with `glyph_style="geometric"`; both serialize as
`type="ascii"`. Internal `ColourTransform` is an execution representation.
Advanced authoring has 21 typed helpers and generic `add_effect` for all 23;
Motion Tile and Radial Blur use the generic path.

`layer.effects.add(effect)` and `project.post_effects.add(effect)` copy the
value and return the attached handle. Later edits to the input value do not edit
that attachment. IDs must be unique within a stack. Scope is validated: a
clip-only effect cannot be attached to the project stack.

## Processing order and time

The [processing diagram](../concepts/effects-transitions-and-animation.md#processing-stages)
shows source sizing/crop → pre-transform effects → transform → post-transform
effects → owned masks → track matte → opacity/blend. Effects preserve authored
order within their intrinsic stage. Motion Tile is the sole pre-transform effect;
ordinary image effects run after transform. Project post-effects run after
compositing the root output. A group layer's effects process its child result.

Ordinary untimed layer effect tracks use layer-local seconds. CameraShake with
a delayed `active_interval` uses effect-local time since that interval began. Global effect tracks use the project
span, including explicit background tails and automatic audio extensions. Pure
validation checks malformed tracks without probing media; preflight checks bounds
that depend on audio duration. An effect does not change layer start or duration.

Numbers must be finite. Bindable scalar properties accept numbers or
`ScalarProperty`, keyframes and signals. Plain tracks accept keyframes without
signals. Motion Tile's `tile_center` and Radial Blur's `center` are dynamic point
properties with a base, keyframes, uniform modifiers and separate x/y modifiers.
Use the generated tables to distinguish static controls from bindable properties.
Brightness is additive (`0` is neutral); Contrast and Saturation are factors
(`1` is neutral, saturation `0` is grayscale). Colors are canonical `#RRGGBB` or `#RRGGBBAA`.

## Motion Tile

Motion Tile is clip-only, at most one per layer, and accepts image, video, shape,
text and group sources. Solid color, Spectrum2D and particle sources are rejected.
It repeats the already-sized/cropped source through a virtual region before the
transform; it does not expand the composition canvas or allocate a larger source
frame. `mirror_edges=True` alternates mirrored tiles. Alpha is sampled with RGB;
transparent source pixels remain transparent. See [sampling and seam rules](../development/effect-algorithms.md).

Expanding the virtual region preserves the sized/cropped source's layer anchor
and sampling phase. `tile_center` distributes the added extent: `(0, 0)` extends
right/down, `(0.5, 0.5)` equally on both sides, and `(1, 1)` left/up. It does not
move the original source pattern. See the
[anchor coverage example](#motion-tile-anchor-coverage).

### Anchor coverage { #motion-tile-anchor-coverage }

A 640×360 opaque rectangle at scale 0.7 and centered position/anchor covers
`(96,54)`–`(544,306)` at 100% (exclusive right/bottom bounds). With a centered
tile center, both 200% and 400% cover the entire canvas:

```python
from vestra import Project, Rectangle
from vestra.effects import MotionTile

project = Project(size=(640, 360), fps=30, duration=1, background="#ff00ff")
layer = project.root.add(Rectangle(width=640, height=360, fill="#93cbb5"), scale=0.7)
layer.effects.add(MotionTile(output_width_percent=400, output_height_percent=400))
project.render("tile-centered.mp4", backend="cpu", overwrite=True)
```

The layer anchor uses the original sized/cropped dimensions. Expansion changes
only virtual sampling bounds; it preserves the source pattern's placement and
phase. `tile_center` controls which sides receive the added extent. This fixes
[#103](https://github.com/evgen2571/vestra/issues/103), where the affine anchor
incorrectly used expanded dimensions and left 27.75% uncovered even at 400%.

The catalog keeps `anchor=(0,0)` and places the nominal image's top-left explicitly
on **both** baseline and treatment. This remains valid without a workaround.
Its outlined original bounds remain
fixed while a 300% mirrored region supplies edge pixels. Tests sample multiple
times and reject uncovered pixels in the treated viewport. The outline itself
is a separate transformed rectangle and is never tiled.

## Radial Blur

The current implementation lowers Radial Blur to centered zoom sampling with 16
taps. `amount` is its radius in output pixels, and `center` supports point
keyframes and bindings. It does not implement rotational/tangential blur.
ZoomBlur exposes a static anchor, selectable direction and sample count instead.
The catalog shows the current behavior rather than inferring it from the name.

## Palettes and dithering

Palettes contain 2–16 opaque colors. Alpha is preserved; transparent palette
entries are rejected. `amount=0` preserves the input, and `amount=1` applies the
full mapping. The [palette guide](../guides/python/palettes-and-dithering.md)
explains choosing a mode and preparing a useful input.

| Mode | Meaning |
| --- | --- |
| `gradient`, `nearest` | Tonal mapping; author colors dark to light. Vestra does not sort them. |
| `rainbow` | Generated hue ramp with increasing brightness. |
| `nearest_rgb`, `nearest_hue`, `nearest_oklab` | Match arbitrary colors in encoded RGB, integer HSV or approximate Oklab. Authored order breaks ties; exact RGB matches are retained. |
| `rgb_channels` | Quantize each encoded RGB channel to `levels` (2–256); palette, phase and period do not affect the result. |

`stops=None` gives uniform tonal positions. Custom stops require one position per
color, strictly increasing from 0 to 1, distinct after rounding to `1/65280`.
They are valid only for `gradient`/`nearest`. RGB is the default interpolation;
Oklab changes gradient interpolation and authored palette motion. Dither still
selects discrete evaluated colors. Matching/interpolation are deterministic
approximations, not full color management.

Bayer matrices and the seamless 32×32 blue-noise tile are fixed in output pixels.
`scale` enlarges the threshold pattern; `seed` transforms the blue-noise tile
(and is ignored by Bayer). It does not generate an independent texture.
`strength=0` selects nearest quantization. `input_scale` independently controls
analysis cells; `input_filter` selects area, nearest or linear analysis.

Input detail runs before exposure/gamma, then analysis and quantization. Gamma
above one lifts midtones; exposure can clip highlights. Final `amount` blends
against the original input, not the tone-adjusted input. Neutral controls preserve
the original path. Active detail adds three full-resolution passes, tone adds one,
and analysis adds one; coarser cells do not shrink physical texture storage.
See [numerical rules and pass costs](../development/effect-algorithms.md).

## Halftone, PixelSort and CRT

Halftone uses alpha-weighted means over rotated output-space cells; transparent
RGB does not contribute. RGB mode offsets the three screens by 60°. Softness
antialiases dot coverage. The original pixel alpha is retained.

PixelSort sorts bounded eligible runs inside fixed horizontal/vertical blocks,
including threshold endpoints. Transparent or ineligible pixels break runs;
equal luminance keeps original order. Reversed authored thresholds are invalid;
crossed evaluated thresholds select no pixels. Threshold crossings may look abrupt.

CRT uses inverse-curvature sampling, integrated scanlines, fixed output-pixel
masks and continuous seeded grain/jitter. It does not create alpha. Periodic
procedural animation is independent of source/audio playback. Recipes return
ordinary editable effect chains; see [composed treatments](../guides/python/effects.md#apply-a-monitor-recipe).

<span id="cinematic-ascii-and-pseudo-ascii"></span>

## ASCII and pseudo-ASCII

Both styles use the same output-space cell analysis. `PseudoAscii` draws geometric
density-ranked marks; `Ascii` uses prepared glyphs. Cells round to whole pixels,
so animating cell size is discrete. Animate `amount` or `source_mix` for a smooth
introduction. `amount=0` or `source_mix=1` preserves the input. Output alpha is
intersected with the original alpha; transparent background permits glyph-only
coverage. See [the ASCII guide](../guides/python/ascii.md).

Fonts use the first face and independent Unicode scalars, without fallback,
ligatures, combining or contextual shaping. Missing/control/invisible non-space
glyphs fail preparation with `VESTRA-ASCII-GLYPH`. Python paths register Font
assets; canonical JSON uses asset IDs. Relative paths use the project base
directory. The bundled font notice is [DejaVuSans.txt](https://github.com/evgen2571/vestra/blob/develop/licenses/DejaVuSans.txt).

**Legacy JSON exception:** schema v1 marks `glyph_style`, `mode` and `color_mode`
optional with defaults, while the Rust parser requires all three. Include them
explicitly in JSON. Python authoring supplies them. Generation preserves this
known mismatch; it does not reconcile it.

## Periods and backend support

A supported `period` adds owner-local time divided by that period to phase.
The procedural component repeats when other keyframes/signals stay unchanged;
source video, audio and the complete composition do not automatically loop.
Random-access evaluation does not depend on a previous frame.

All catalog previews use CPU. Consult [feature support](feature-support.md) for
specific WGPU evidence; dispatch availability does not establish visual parity.
The [algorithm reference](../development/effect-algorithms.md) retains exact
rounding, alpha, boundary, atlas and resource behavior.
