# Presets and flashes

Presets are convenience values, not a separate rendering primitive. Current
preset kinds are `slow_drift`, `zoom_punch`, `impact`, `heavy_impact`, and
`focus_reveal`. Each has `intensity` from `0` to `2`, non-negative `start`, and
optional positive `duration`. `impact` and `heavy_impact` require a 64-bit
unsigned `seed`; the other kinds reject `seed`.

The high-level `PresetCollection` permits at most one preset on a layer and
currently accepts presets only on Image layers. Presets lower to canonical
image-clip preset values.

A flash is a root-owned color overlay with `id`, `start`, positive `duration`,
`colour`, `opacity`, `fade_in`, `fade_out`, and `layer`. Fades must fit inside
the duration. Flash placement uses project timeline seconds. Python exposes
`Flash` and `FlashCollection`; canonical JSON stores flashes under `visual`.
Validation checks timing, opacity, colors, ids, and fade fit.
