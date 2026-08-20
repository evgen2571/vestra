# Solid-color source

Python calls the source `Color` and keeps `SolidColor` as an alias. The
canonical tag is `solid_color` with a `colour` value. Colors are canonical
`#RRGGBB` or `#RRGGBBAA` strings, including alpha when supplied.

The source fills its layer surface. Canvas size comes from project output;
layer timing, transform, opacity, animation, effects, and transitions remain
layer contracts. Solid color is supported by the canonical model, Python, CPU,
and WGPU renderers.
