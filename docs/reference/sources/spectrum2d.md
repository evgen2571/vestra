# Spectrum2D source

`Spectrum2D(*, preset=None, band_count=None, min_hz=None, max_hz=None, ... )`
is an audio-reactive source. Omitted fields resolve through the selected preset
or current defaults. Its canonical tag is `spectrum2d`.

| Field | Default | Contract |
| --- | --- | --- |
| `band_count` | 24 | Integer from 1 through 48. |
| `min_hz`, `max_hz` | 40, 16000 | Hz; ordered and bounded by available master-audio Nyquist. |
| `sensitivity` | 8 | Finite scalar. |
| `attack_seconds`, `release_seconds` | 0.020, 0.150 | Non-negative seconds. |
| `x`, `y`, `width`, `height` | 0.10, 0.70, 0.80, 0.25 | Normalized layout geometry. |
| `bar_gap_ratio`, `min_bar_height_ratio` | 0.20, 0 | Valid normalized ratios. |
| `colour`, `layout`, `gradient` | source defaults | Canonical colour and typed layout/gradient values. |

Spectrum2D requires usable prepared master audio analysis. It does not read an
audio file itself. Preflight validates analysis availability, frequency bounds,
layout, and colours. CPU support has dedicated source/renderer paths. WGPU has
a source path, but exact visual parity beyond covered tests is not fully
verified. It is not a direct transform or transition endpoint.
