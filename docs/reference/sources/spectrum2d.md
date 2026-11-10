# Spectrum2D source

`Spectrum2D` draws linear or radial frequency bands from prepared master audio.
For a silent linear/radial comparison with visible input timing and tuning
guidance, see [draw an audio spectrum](../../guides/python/spectra.md). Its input
is prepared master frequency analysis, separate from scalar signal bindings.

## API signature

```text title="Source signature (presentation keywords omitted)"
Spectrum2D(*, preset=None, band_count=None, min_hz=None, max_hz=None,
           sensitivity=None, attack_seconds=None, release_seconds=None,
           x=None, y=None, width=None, height=None, bar_gap_ratio=None,
           colour=None, min_bar_height_ratio=None, layout=None, gradient=_UNSET)
```

`preset` is one of `classic`, `dense`, `neon`, `mirror`, `center_out`, `circle`, `neon_circle` or `arc`. Omitted fields resolve in this order: base defaults, preset values, explicit arguments. Passing `gradient=None` explicitly disables a preset gradient.

## Frequency analysis and response

| Argument | Base default | Contract |
| --- | --- | --- |
| `band_count` | `24` | Integer 1 through 48. |
| `min_hz`, `max_hz` | `40.0`, `16000.0` | Finite Hz; `0 < min_hz < max_hz <= 24000`, the prepared master-audio Nyquist limit. |
| `sensitivity` | `8.0` | Finite positive scalar. |
| `attack_seconds`, `release_seconds` | `0.020`, `0.150` | Non-negative seconds. |

## Layout and color

All positions and dimensions below are normalized to the containing canvas,
not source-local pixels. The rectangle must satisfy `x >= 0`, `y >= 0`,
`width > 0`, `height > 0`, `x + width <= 1` and `y + height <= 1`.

| Argument | Base default | Contract |
| --- | --- | --- |
| `x`, `y`, `width`, `height` | `0.10`, `0.70`, `0.80`, `0.25` | Finite normalized layout rectangle. |
| `bar_gap_ratio` | `0.20` | Finite ratio in `[0, 1)`. |
| `min_bar_height_ratio` | `0.0` | Finite ratio in `[0, 1]`. |
| `colour` | `"#ffffff"` | Canonical `#RRGGBB` or `#RRGGBBAA`. |
| `layout` | `Spectrum2DLinearLayout()` | Linear or radial layout; signatures below. |
| `gradient` | none | `Spectrum2DGradient(start_color, end_color, direction)`; direction is `along_bar` or `across_bands`. |

```text title="Layout signatures"
Spectrum2DLinearLayout(anchor="bottom", band_mapping="forward")
Spectrum2DRadialLayout(inner_radius_ratio=0.55, start_angle_degrees=0.0,
                       sweep_angle_degrees=360.0, direction="outward",
                       band_mapping="forward")
```

Linear anchors are `bottom`, `top` or `center`; linear mappings are `forward`, `reverse` or `center_out`. Radial layout requires `0 <= inner_radius_ratio < 1`, `0 < sweep_angle_degrees <= 360`, direction `outward`/`inward`/`both`, and radial mapping `forward`/`reverse`.

## Validation, preparation and support

Construction validates field types/ranges and layout/gradient objects. Canonical
semantic validation additionally checks the combined layout rectangle, ordered
frequency interval and presence of authored master audio. Preparation must then
resolve usable master-audio analysis. Muting output audio does not remove that
analysis; see [audio output policy](../audio.md#analysis-and-output-policy).

Spectrum2D lowers to `spectrum2d`. Presets expand into source values and any
preset effects; the preset name is not a canonical source field. Gradient
`start_color`/`end_color` become `start_colour`/`end_colour` in JSON.

Layers own placement timing, effects, opacity and blending. Spectrum2D is not a
direct canonical transform or transition endpoint; the high-level Python layer
supplies presentation through an adapter. CPU has dedicated support; WGPU source
dispatch exists, but full visual parity is not verified.

Python constructors also accept [presentation transform defaults](../../guides/python/sources-and-layers.md),
which are copied into each placement and can be individually overridden by `add()`.
