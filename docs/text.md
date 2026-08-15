# Text Sources

`Text` is a static visual Source. It requires a file-backed TTF or OTF font
asset so the same font data is used on every machine:

```python
from vestra import Text

title = Text(
    "Vestra",
    font="assets/Inter-Bold.ttf",
    font_size=96,
    fill="#ffffff",
)
layer = scene.add(title, start=0, duration=5)
layer.transform.scale = (0.8, 0.8)
```

Text supports Unicode strings, explicit newlines, word/character wrapping with
`max_width`, and `left`, `center`, or `right` alignment. `line_spacing` is a
multiplier relative to `font_size`: the line height is `font_size ×
line_spacing` (`1.0` means a line height equal to `font_size`, `1.2` means
20% larger, and `2.0` doubles it). `letter_spacing` is additional tracking in
source-local pixels.

Text-local properties are static in v1C. Layer timing, transforms, opacity,
blend mode, effects, transitions, Groups, and nested Compositions remain
ordinary Layer features. Missing or invalid font files fail during validation
or preparation; Vestra does not search system fonts or silently fall back.
TTF and OTF files supported by the selected shaping engine are the intended v1C
font formats. Empty and whitespace-only strings are valid transparent Text
Sources; they retain deterministic line metrics while using a minimal raster.
