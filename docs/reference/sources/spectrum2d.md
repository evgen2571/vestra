# Spectrum2D source

`Spectrum2D` draws audio-reactive bars or smooth waves from prepared master audio.
For a silent linear/radial comparison with visible input timing and tuning
guidance, see [draw an audio spectrum](../../guides/python/spectra.md). Its input
is prepared master frequency analysis, separate from scalar signal bindings.

## API signature

```text title="Source signature summary (presentation keywords omitted)"
Spectrum2D(*, preset=None, band_count=None, min_hz=None, max_hz=None,
           sensitivity=None, attack_seconds=None, release_seconds=None,
           x=None, y=None, width=None, height=None, bar_gap_ratio=None,
           colour=None, min_bar_height_ratio=None, layout=None, gradient=_UNSET,
           style=None, wave=_UNSET, response_power=None, soft_knee=_UNSET)
```

`preset` is one of `classic`, `dense`, `neon`, `mirror`, `center_out`, `circle`, `neon_circle`, `arc` or `stereo_glow`. Omitted fields resolve in this order: base defaults, preset values, explicit arguments. Passing `gradient=None` explicitly disables a preset gradient.

## Frequency analysis and response

| Argument | Base default | Contract |
| --- | --- | --- |
| `band_count` | `24` | Integer 1 through 48. |
| `min_hz`, `max_hz` | `40.0`, `16000.0` | Finite Hz; `0 < min_hz < max_hz <= 24000`, the prepared master-audio Nyquist limit. |
| `response_power` | `1.0` | Exponent in `(0, 1]` applied to each band’s power before sensitivity. Smaller values lift quiet bands. |
| `soft_knee` | none | Threshold in `[0, 1)` above which the response eases toward 1. The default hard-clips at 1. |
| `sensitivity` | `8.0` | Finite positive scalar. |
| `attack_seconds`, `release_seconds` | `0.020`, `0.150` | Non-negative seconds. |

## Layout and color

The layout rectangle uses fractions of the containing composition’s width and
height. It must satisfy `x >= 0`, `y >= 0`,
`width > 0`, `height > 0`, `x + width <= 1` and `y + height <= 1`.

| Argument | Base default | Contract |
| --- | --- | --- |
| `x`, `y`, `width`, `height` | `0.10`, `0.70`, `0.80`, `0.25` | Finite normalized layout rectangle. |
| `bar_gap_ratio` | `0.20` | Finite ratio in `[0, 1)`. |
| `min_bar_height_ratio` | `0.0` | Finite ratio in `[0, 1]`. |
| `colour` | `"#ffffff"` | Canonical `#RRGGBB` or `#RRGGBBAA`. |
| `layout` | `Spectrum2DLinearLayout()` | Linear, radial, ellipse or polyline. |
| `gradient` | none | `Spectrum2DGradient(start_color, end_color, direction)`; direction is `along_bar` or `across_bands`. |

```text title="Layout signatures"
Spectrum2DLinearLayout(anchor="bottom", band_mapping="forward")
Spectrum2DRadialLayout(inner_radius_ratio=0.55, start_angle_degrees=0.0,
                       sweep_angle_degrees=360.0, direction="outward",
                       band_mapping="forward")
Spectrum2DEllipseLayout(radius_x, radius_y, start_angle_degrees=0.0,
                        sweep_angle_degrees=360.0, direction="outward",
                        band_mapping="forward")
Spectrum2DPathLayout(points, closed=False, direction="left", band_mapping="forward")
```

Linear anchors are `bottom`, `top` or `center`; linear mappings are `forward`, `reverse` or `center_out`. Radial layout requires `0 <= inner_radius_ratio < 1`, `0 < sweep_angle_degrees <= 360`, direction `outward`/`inward`/`both`, and radial mapping `forward`/`reverse`.

An ellipse centers in the layout rectangle. Its radii are fractions of that
rectangle’s width and height, each in `(0, 0.5]`. Angles run clockwise from
the top. Use a start of 270° and sweep of 180° for the upper half, or a sweep
of 360° for a closed ellipse. Bars and waves can grow outward, inward or both.

A polyline takes 2–256 `(x, y)` pairs in rectangle coordinates, with each
coordinate in `[0, 1]` and Y increasing downward. Canonical JSON uses
`{"x": ..., "y": ...}` for each point. Closed paths have at least three
points and join the last to the first automatically. Consecutive points must
differ. `left` and `right` are relative to traversal. For a segment running
from left to right, `left` grows upward. On both new layouts, rectangle height in
pixels is the maximum growth and `both` divides that distance equally.

Bands occupy equal-length cells measured in local pixels. `forward` begins with
the lowest band and `reverse` begins with the highest. The first point or arc
angle starts the first cell. Band samples are at cell centers. At an open end,
the wave extends with the nearest band’s level. At a closed seam it interpolates
between the last and first bands. Polyline normals follow each segment and
bisect turns at vertices. At a 180° turn the outgoing segment sets the normal.

## Wave style and stereo

Choose `style="bars"` (the default), `"wave"` or `"both"`. A wave smoothly
joins band tips and fills back to the baseline, including on curved and closed
paths. Combined style draws bars beneath the waves. These settings are
available in `Spectrum2DWave(...)` and in the canonical `wave` object.

| Option | Default | Meaning |
| --- | --- | --- |
| `stroke_width` | `2.0` | Stroke width in local pixels, in `(0, 64]`. |
| `color` | source colour | Stroke, fill and halo tint. In JSON use `colour`. |
| `fill_opacity` | `0.18` | Opacity multiplier for the area between the tip and baseline. |
| `glow_radius` | `0.0` | Soft halo radius in local pixels, in `[0, 32]`. |
| `glow_opacity` | `0.45` | Halo opacity multiplier. |
| `bar_opacity` | `1.0` | Bar opacity multiplier in combined style. |
| `stereo` | none | `Spectrum2DStereo(...)` enables one wave per Master channel. |

Opacity multipliers are in `[0, 1]`. `Spectrum2DStereo` accepts `left_color`
and `right_color` (JSON `left_colour` and `right_colour`). Both fall back to the
wave tint. `left_weight` and `right_weight` multiply opacity for the channel's
stroke, fill and halo and default to 0.65 and 1.0 respectively. The stereo
master mix remixes mono inputs into both channels at equal level. Bars in
combined mode use the existing average of channel power before response
shaping. Silent waves disappear. Set `min_bar_height_ratio` to keep inactive
bars visible.

The `stereo_glow` preset uses a straight baseline, two tinted waves, a soft
halo, translucent fill and subdued bars. It uses 32 bands from 100 to 6000 Hz,
a response exponent of 0.25, sensitivity 2.3, a knee at 0.6, 35 ms attack and
330 ms release. It expands into editable source settings. Set `style="wave"`
for the same preset without bars, or pass `wave=Spectrum2DWave(...)` to
replace its styling.

## Transform and clip the result

High-level Python layer transforms move the path, output and attached masks
together through the composition adapter. In canonical projects use a
transformed group around the spectrum. A filled shape mask clips the source
after layer effects, including its wave fill and halo. Invert the mask to keep
the exterior. Growth direction determines which side of the baseline the bars
and waves occupy. Mask coverage also clips stroke edges, effects and any growth
that reaches the other side of the silhouette.

To align an ellipse mask within a local canvas of size `W × H`, position it
at `(x + width / 2, y + height / 2)` with the default center anchor. Set its
pixel width to `2 * radius_x * width * W` and its pixel height to
`2 * radius_y * height * H`. Use a filled ellipse with no stroke for the
coverage. The [spectrum guide](../../guides/python/spectra.md) includes runnable
arc and stereo examples.

## Validation, preparation and support

Construction validates field types/ranges and layout/gradient objects. Canonical
semantic validation additionally checks the combined layout rectangle, ordered
frequency interval and presence of authored master audio. Preparation must then
resolve usable master-audio analysis. Muting output audio does not remove that
analysis; see [audio output policy](../audio.md#analysis-and-output-policy).

The Rust SDK exports `Spectrum2D`, its layouts and style types, and `Point` for
polyline coordinates. Both Python authoring paths use the same settings.

Spectrum2D lowers to `spectrum2d`. Presets expand into source values and any
preset effects; the preset name is not a canonical source field. Gradient
`start_color`/`end_color` become `start_colour`/`end_colour` in JSON.

Layers own placement timing, effects, opacity and blending. Spectrum2D is not a
direct canonical transform or transition endpoint; the high-level Python layer
supplies presentation through an adapter. CPU and WGPU share the rasterizer for path layouts and waves. WGPU uploads
that source raster and runs layer effects, masking and compositing on the
selected adapter.

Python constructors also accept [presentation transform defaults](../../guides/python/sources-and-layers.md),
which are copied into each placement and can be individually overridden by `add()`.
