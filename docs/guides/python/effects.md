# Add effects

Visual effects attach to a layer or to the project output. Layer effects are
processed with that layer. Project `post_effects` apply to the rendered visual
output.

```python
from vestra import Brightness, GaussianBlur
from vestra.sources import Image

layer = project.root.add(Image("assets/photo.png"), duration=5)
layer.effects.add(Brightness(0.1))
# Keep the attached effect handle so its radius can be animated.
blur = layer.effects.add(GaussianBlur(3.0))
blur.radius.keyframe(0, 0.0)
blur.radius.keyframe(1.0, 3.0)

# Post effects process the final composition, after its layers.
project.post_effects.add(Brightness(0.05))
```

Motion Tile is intended for the common edge-filling edit where a layer is
zoomed, rotated, or shaken:

```python
from vestra import ChromaticAberration, DirectionalBlur, MotionTile

layer.effects.add(MotionTile(200, 200, mirror_edges=True))
layer.transform.scale = (1.25, 1.25)
layer.effects.add(DirectionalBlur(12, 45))
layer.effects.add(ChromaticAberration(4, 0))
```

Motion Tile runs before the layer transform. Directional Blur, Radial Blur,
Chromatic Aberration, and the other image effects run after it. Its percentage
parameters describe the virtual tiled source region, while the composition
canvas size is unchanged.

`EffectStack` preserves insertion order. Use layer effects for content-specific
processing and post effects for a whole-project look. Effect parameters that
are exposed as properties can be animated or bound to a signal.

Choose a representative effect first, then validate. Exact parameter ranges
and the complete effect catalog belong to the API reference. An effect does not
change a layer's start or duration.


For fine palette dithering, use a dark-to-light set of opaque colors and an
output-pixel pattern scale of one:

```python
from vestra.effects import OrderedDither, PaletteMap, PaletteMode

palette = ("#071827", "#27565d", "#69b49c", "#fff0c0")
dither = layer.effects.add(OrderedDither(palette, matrix="bayer8", scale=1))
dither.strength.keyframe(0, 0.25)
dither.strength.keyframe(1, 1)

# An explicit two-second procedural color loop.
color = project.post_effects.add(PaletteMap(mode=PaletteMode.RAINBOW, period=2))
color.amount = 0.25
color.phase.bind(project.audio.signal.rms(), operation="add")
```

Bayer and blue-noise patterns stay fixed in output pixel coordinates. For a
less regular fine texture, use `OrderedDither(palette, matrix="blue_noise",
seed=37, scale=1)`. The seed shifts/reflects a fixed seamless 32×32 tile; it does
not change over time. Bayer defaults and output remain unchanged. To adjust tonal
separation before quantization, place `ColorAdjust` or `Contrast` earlier in
the stack. Palette Map's `gradient` mode gives smooth coloring; `nearest`
gives discrete tonal bands. Dither always quantizes to discrete colors.
Use `mode="nearest_rgb"` to retain source colors with an arbitrary palette, or
`mode="nearest_hue"` to emphasize hue and saturation. These modes also work with
`PaletteMap`. Chromatic dithering mixes the two closest colors; colors need not
be ordered from dark to light. Compare both on the generated moving footage:

```bash
uv run python examples/showcase/stylized-effects/main.py --look dither-blue-noise --palette ember --mode nearest_rgb --smoke
uv run python examples/showcase/stylized-effects/main.py --look dither-blue-noise --palette ember --mode nearest_hue --smoke
```

For an RGB color cube instead of an authored palette, use
`OrderedDither(mode="rgb_channels", levels=4, matrix="blue_noise")` or
`PaletteMap(mode="rgb_channels", levels=4)`. Two levels give eight RGB colors;
four give 64 colors; 256 preserve the source bytes. Palette phase does not alter
this mode. In the showcase, select `--mode rgb_channels --levels 4`.

Use `mode="nearest_oklab"` for perceptual palette matching. It interprets source
bytes as sRGB and uses a fixed-point Oklab approximation, matching lightness and
chroma together. It preserves exact palette inputs and retains authored order.
Compare it with `nearest_rgb` on the same footage and palette using the showcase's
`--mode nearest_oklab` option. Neither mode alters the selected palette bytes.

An explicit `period` loops procedural palette phase; the source clip and audio
continue on their normal timelines. See the [effect reference](../../reference/effects.md)
for exact ranges and the [palette example](../../../examples/python/high-level/13_palette_dither.py)
for an asset-free recipe.

Halftone, sorting and CRT can be attached to video layers, groups or the global
stack. Numeric properties support the same keyframes and signals:

```python
from vestra.effects import Halftone, PixelSort, Crt
from vestra.effects.recipes import analog_monitor

print_effect = layer.effects.add(Halftone(mode="source", cell_size=6, softness=0.5))
print_effect.amount.keyframe(0, 0)
print_effect.amount.keyframe(1, 1)
layer.effects.add(PixelSort(direction="vertical", segment_length=64, amount=0.4))
project.post_effects.add(Crt(period=4, seed=7, jitter=0.2))

# An optional reusable chain, with independently editable ordinary effects.
for effect in analog_monitor(period=4):
    project.post_effects.add(effect)
```

Halftone analyzes whole rotated cells with alpha-aware area means. Sorting uses
bounded runs, stable equal-luminance ties and deliberate hard threshold changes.
CRT grain and jitter evolve continuously, so evaluating frames out of order
preserves their animation. A procedural CRT period does not loop footage/audio.
Modes, colors and integer block sizes are discrete configuration choices;
animate intensity to introduce a look smoothly. See the
[synthetic looping showcase](../../../examples/showcase/stylized-effects/README.md)
and the [effect reference](../../reference/effects.md) for parameter bounds.

For cinematic ASCII, choose dark-to-light characters, output-pixel cell sizes
and optional colors/source blending. The same effect works on images, footage,
compositions and global output:

```python
from vestra.effects import Ascii, PseudoAscii, CHARACTER_SETS

ascii_effect = layer.effects.add(Ascii(
    CHARACTER_SETS["dense"], cell_width=8, cell_height=12,
    mode="hybrid", color_mode="source", background="#00000000",
    source_mix=.15,
))
ascii_effect.amount.keyframe(0, 0)
ascii_effect.amount.keyframe(1, 1)
project.post_effects.add(PseudoAscii(color_mode="rainbow", period=4))

# Explicit portable font resource; no system-font fallback.
layer.effects.add(Ascii(" .oO#", font="assets/MyFont.ttf", mode="fill"))
```

`font=None` uses the bundled licensed font. Custom font glyph coverage is
validated during preparation; unsupported or invisible characters fail with a
resource diagnostic. Each Unicode scalar is independent, so this effect does
not shape ligatures or combining sequences. Use `background="#00000000"` to
retain glyph-only alpha; `source_mix` blends footage detail in premultiplied
space. Cell sizes, amount, source mix, edge controls and phase support the
existing keyframe/audio-signal properties. Palette/rainbow periods repeat the
effect's colors, while footage/audio must separately align for a looping video.
See the [ASCII reference](../../reference/effects.md#cinematic-ascii-and-pseudo-ascii)
for area filtering, anchoring, prepared glyph resources and discrete controls.

Nonuniform tonal palettes use `stops` on `PaletteMap` or `OrderedDither`:

```python
from vestra.effects import OrderedDither

look = OrderedDither(
    ("#080508", "#351120", "#9e3341", "#efa463", "#fff1c5"),
    stops=(0, 0.12, 0.35, 0.7, 1), matrix="blue_noise", scale=1,
)
look.stops = (0, 0.18, 0.42, 0.8, 1)
look.stops = None  # Restore uniform tonal positions.
```

The advanced `add_palette_map` and `add_ordered_dither` methods accept the same
control. Positions are static, one per color, strictly increasing with endpoints
zero and one, and distinct after rounding to `1/65280`. Native project validation
checks palette cardinality and restricts stops to `gradient`/`nearest` modes.
Phase remains bindable and animates colors through those fixed positions.

Palette interpolation is independently selectable:

```python
from vestra.effects import PaletteMap, PaletteInterpolation

look = PaletteMap(("#ff0000", "#0000ff", "#ffffff"), interpolation="oklab")
look.interpolation = PaletteInterpolation.RGB
```

`OrderedDither` and advanced `add_palette_map`/`add_ordered_dither` expose the same
static control. RGB is the compatible default; Oklab affects gradient mapping and
animated authored palette colors. Dither output remains discrete palette colors.
Amounts and phase continue to support keyframes and signal binding.


Use entering-signal tone controls to lift shadows without changing the source
used for a partial blend:

```python
from vestra.effects import OrderedDither

look = OrderedDither(("#080508", "#9e3341", "#efa463", "#fff1c5"),
                     matrix="blue_noise", input_exposure=0.5, input_gamma=1.5)
look.input_gamma.keyframe(0, 1)
look.input_gamma.keyframe(2, 1.5)
```

`PaletteMap` and both advanced authoring methods expose the same controls.
Exposure and gamma can bind to signals. Neutral values preserve existing output;
the final `amount` always blends against the original image.

Preserve small edges before quantization with `input_detail=1.5` and
`input_detail_radius=1`. For broader local contrast, try `0.5` and `8` instead.
Both controls are bindable on `PaletteMap`, `OrderedDither` and their advanced
authoring counterparts:

```python
look.input_detail = 1.5
look.input_detail_radius = 1
look.input_detail.keyframe(0, 0)
look.input_detail.keyframe(2, 1.5)
```

Detail runs before exposure/gamma and adds three full-resolution passes when
active. Its default zero amount preserves existing output.
