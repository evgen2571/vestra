# Spectrum2D source

```python
Spectrum2D(*, preset=None, band_count=None, min_hz=None, max_hz=None,
           sensitivity=None, attack_seconds=None, release_seconds=None,
           x=None, y=None, width=None, height=None, bar_gap_ratio=None,
           colour=None, min_bar_height_ratio=None, layout=None, gradient=_UNSET)
```

`preset` is one of `classic`, `dense`, `neon`, `mirror`, `center_out`, `circle`, `neon_circle` or `arc`. Omitted fields resolve in this order: base defaults, preset values, explicit arguments. Passing `gradient=None` explicitly disables a preset gradient.

| Argument | Base default | Contract |
| --- | --- | --- |
| `band_count` | `24` | Integer 1 through 48. |
| `min_hz`, `max_hz` | `40.0`, `16000.0` | Finite Hz, ordered; `max_hz` cannot exceed prepared master-audio Nyquist. |
| `sensitivity` | `8.0` | Finite scalar. |
| `attack_seconds`, `release_seconds` | `0.020`, `0.150` | Non-negative seconds. |
| `x`, `y`, `width`, `height` | `0.10`, `0.70`, `0.80`, `0.25` | Normalized layout rectangle. |
| `bar_gap_ratio`, `min_bar_height_ratio` | `0.20`, `0.0` | Normalized ratios. |
| `colour` | `"#ffffff"` | Canonical `#RRGGBB` or `#RRGGBBAA`. |
| `layout` | `Spectrum2DLinearLayout()` | `Spectrum2DLinearLayout(anchor="bottom", band_mapping="forward")`, or `Spectrum2DRadialLayout(inner_radius_ratio=0.55, start_angle_degrees=0.0, sweep_angle_degrees=360.0, direction="outward", band_mapping="forward")`. |
| `gradient` | none | `Spectrum2DGradient(start_color, end_color, direction)`; direction is `along_bar` or `across_bands`. |

Linear anchors are `bottom`, `top` or `center`; linear mappings are `forward`, `reverse` or `center_out`. Radial layout requires `0 <= inner_radius_ratio < 1`, `0 < sweep_angle_degrees <= 360`, direction `outward`/`inward`/`both`, and radial mapping `forward`/`reverse`.

Spectrum2D lowers to `spectrum2d` and requires prepared master-audio analysis. It is not a direct transform or transition endpoint. CPU has dedicated support; WGPU source dispatch exists, but full visual parity is not verified.
