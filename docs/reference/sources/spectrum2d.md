# Spectrum2D source

`Spectrum2D` is an audio-reactive visual source. Its public properties include
`band_count` (default `24`, range `1..=48`), `min_hz` (default `40`), `max_hz`
(default `16000`, bounded by the master-audio Nyquist frequency), `sensitivity`
(default `8`), attack `0.020` seconds, release `0.150` seconds, normalized
`x=0.10`, `y=0.70`, `width=0.80`, `height=0.25`, `bar_gap_ratio=0.20`, and
`min_bar_height_ratio=0`. Layout and gradient values come from Spectrum2D
authoring types. The canonical tag is `spectrum2d`.

The source requires usable audio analysis during preparation. Validation checks
band count, frequency ordering and Nyquist, normalized geometry, colors, and
layout values. CPU support is covered by source and renderer tests. WGPU
support follows the current WGPU source path and should not be inferred from
the enum alone.
