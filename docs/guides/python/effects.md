# Add effects

Visual effects attach to a layer or to the project output. Layer effects are
processed with that layer. Project `post_effects` apply to the rendered visual
output.

Use your own `assets/photo.png`. Start with one brightness adjustment. The
animation section continues this project; later treatments start fresh so you
can compare them without accumulating the earlier effects.

```python
from vestra import Brightness, GaussianBlur, Image, Project

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))
photo.effects.add(Brightness(amount=0.1))
```

## Animate an effect

Keep the handle returned by `add()` to edit the effect attached to this photo:

```python
blur = photo.effects.add(GaussianBlur(radius=3))
blur.radius.keyframe(0, 0)
blur.radius.keyframe(1, 3)
```

The blur grows from zero to three pixels during the first second. To brighten
all layers after compositing, attach the effect to the project instead:

```python
project.post_effects.add(Brightness(amount=0.05))
```

Both stacks copy effects on attachment. You can animate a reusable effect
before adding it; use the returned handle for later changes to that placement.
See [effect ownership](animation.md#effect-ownership).

## Fill the edges of a moving image

Motion Tile is intended for the common edge-filling edit where a layer is
zoomed, rotated, or shaken:

```python
from vestra import ChromaticAberration, DirectionalBlur, MotionTile

tiles = MotionTile(
    output_width_percent=200,
    output_height_percent=200,
    mirror_edges=True,
)
photo.effects.add(tiles)
photo.transform.scale = 1.25
```

Optional blur and colour splitting go after the transform:

```python
photo.effects.add(DirectionalBlur(radius=12, angle_degrees=45))
photo.effects.add(ChromaticAberration(amount=4, angle_degrees=0))
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

## Reduce colours with dithering

For fine palette dithering, use a dark-to-light set of opaque colors and an
output-pixel pattern scale of one:

```python
from vestra.effects import OrderedDither

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

palette = ("#071827", "#27565d", "#69b49c", "#fff0c0")
dither = photo.effects.add(OrderedDither(palette=palette, matrix="bayer8", scale=1))
dither.strength.keyframe(0, 0.25)
dither.strength.keyframe(1, 1)
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

To add a gentle rainbow treatment to the complete composition, use a separate
post-effect. This is an alternative look rather than a required part of dithering:

```python
from vestra.effects import PaletteMap, PaletteMode

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

rainbow = PaletteMap(mode=PaletteMode.RAINBOW, period=2, amount=0.25)
project.post_effects.add(rainbow)
```

An explicit `period` loops procedural palette phase; the source clip and audio
continue on their normal timelines. See the [effect reference](../../reference/effects.md)
for exact ranges and the [palette example](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/python/high-level/13_palette_dither.py)
for an asset-free recipe.

## Try halftone, sorting or CRT

Halftone, sorting and CRT can be attached to video layers, groups or the global
stack. Numeric properties support the same keyframes and signals:

```python
from vestra.effects import Halftone

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

print_effect = photo.effects.add(Halftone(mode="source", cell_size=6, softness=0.5))
print_effect.amount.keyframe(0, 0)
print_effect.amount.keyframe(1, 1)
```

For a different treatment, start with a fresh photo layer and attach vertical sorting:

```python
from vestra.effects import PixelSort

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

sorting = PixelSort(direction="vertical", segment_length=64, amount=0.4)
photo.effects.add(sorting)
```

CRT can process the final composition:

```python
from vestra.effects import Crt

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

project.post_effects.add(Crt(period=4, seed=7, jitter=0.2))
```

For a composed monitor treatment, use the recipe instead of that individual CRT
effect. Start with a new project for this alternative. The recipe returns ordinary effects that remain independently editable:

```python
from vestra.effects.recipes import analog_monitor

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))
for effect in analog_monitor(period=4):
    project.post_effects.add(effect)
```

Halftone analyzes whole rotated cells with alpha-aware area means. Sorting uses
bounded runs, stable equal-luminance ties and deliberate hard threshold changes.
CRT grain and jitter evolve continuously, so evaluating frames out of order
preserves their animation. A procedural CRT period does not loop footage/audio.
Modes, colors and integer block sizes are discrete configuration choices;
animate intensity to introduce a look smoothly. See the
[synthetic looping showcase](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/stylized-effects/README.md)
and the [effect reference](../../reference/effects.md) for parameter bounds.

## Turn image detail into characters

For ASCII, choose dark-to-light characters, output-pixel cell sizes
and optional colors/source blending. The same effect works on images, footage,
compositions and global output:

```python
from vestra.effects import Ascii, CHARACTER_SETS

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

ascii_look = Ascii(
    characters=CHARACTER_SETS["dense"],
    cell_width=8,
    cell_height=12,
    mode="hybrid",
    color_mode="source",
    background="#00000000",
    source_mix=0.15,
)
ascii_effect = photo.effects.add(ascii_look)
ascii_effect.amount.keyframe(0, 0)
ascii_effect.amount.keyframe(1, 1)
```

The example uses the bundled font. To use a file you provide instead, create the
effect with that path:

```python
project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

custom_letters = Ascii(characters=" .oO#", font="assets/MyFont.ttf", mode="fill")
photo.effects.add(custom_letters)
```

`PseudoAscii` offers another character-like treatment without a glyph font:

```python
from vestra.effects import PseudoAscii

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

project.post_effects.add(PseudoAscii(color_mode="rainbow", period=4))
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

## Adjust palette spacing and interpolation

Nonuniform tonal palettes use `stops` on `PaletteMap` or `OrderedDither`:

```python
from vestra.effects import OrderedDither

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

palette_look = OrderedDither(
    palette=("#080508", "#351120", "#9e3341", "#efa463", "#fff1c5"),
    stops=(0, 0.12, 0.35, 0.7, 1),
    matrix="blue_noise",
    scale=1,
)
look = photo.effects.add(palette_look)
look.stops = (0, 0.18, 0.42, 0.8, 1)
```

Set `look.stops = None` to restore uniform tonal positions.

The advanced `add_palette_map` and `add_ordered_dither` methods accept the same
control. Positions are static, one per color, strictly increasing with endpoints
zero and one, and distinct after rounding to `1/65280`. Native project validation
checks palette cardinality and restricts stops to `gradient`/`nearest` modes.
Phase remains bindable and animates colors through those fixed positions.

Palette interpolation is independently selectable:

```python
from vestra.effects import PaletteMap, PaletteInterpolation

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

palette_look = PaletteMap(
    palette=("#ff0000", "#0000ff", "#ffffff"), interpolation="oklab"
)
look = photo.effects.add(palette_look)
```

Set `look.interpolation = PaletteInterpolation.RGB` if you want RGB interpolation
instead.

`OrderedDither` and advanced `add_palette_map`/`add_ordered_dither` expose the same
static control. RGB is the compatible default; Oklab affects gradient mapping and
animated authored palette colors. Dither output remains discrete palette colors.
Amounts and phase continue to support keyframes and signal binding.

## Adjust detail before quantization

Use input tone controls to lift shadows without changing the source
used for a partial blend:

```python
from vestra.effects import OrderedDither

project = Project(
    size=(1280, 720),
    fps=30,
    duration=5,
)
photo = project.root.add(Image("assets/photo.png"))

tone_look = OrderedDither(
    palette=("#080508", "#9e3341", "#efa463", "#fff1c5"),
    matrix="blue_noise",
    input_exposure=0.5,
    input_gamma=1.5,
)
look = photo.effects.add(tone_look)
look.input_gamma.keyframe(0, 1)
look.input_gamma.keyframe(2, 1.5)
```

`PaletteMap` and both advanced authoring methods expose the same controls.
Exposure and gamma can bind to signals. Neutral values preserve existing output;
the final `amount` always blends against the original image.

Preserve small edges before quantization with `input_detail=1.5` and
`input_detail_radius=1`. For broader local contrast, try `0.5` and `8` instead.
Both controls are bindable on `PaletteMap`, `OrderedDither` and their advanced
authoring counterparts. Continue with the attached `look` from the tone example:

```python
look.input_detail_radius = 1
look.input_detail.keyframe(0, 0)
look.input_detail.keyframe(2, 1.5)
```

Detail runs before exposure/gamma and adds three full-resolution passes when
active. Its default zero amount preserves existing output.

For a complete fine-detail dither video and ASCII readability comparison, run
[the generated portrait example](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/stylized-effects/reference.py).
Its [guide](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/stylized-effects/README.md#fine-detail-reference-and-ascii-diagnosis)
explains editable tone/detail/filter settings, three arbitrary palettes and the
expected darkness of source-colored ASCII with zero source mixing.

## Render a treatment

After choosing a treatment, render that project:

```python
project.render("photo-treatment.mp4", backend="cpu", overwrite=True)
```

This writes a five-second MP4 to the current directory and replaces it on a
rerun. Change the output name to keep comparisons of different looks.
