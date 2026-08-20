# Image source

`vestra.sources.Image(path, sizing=None, crop=None)` references a raster image.
The path is a non-empty string or path-like value. The canonical source tag is
`image`; asset loading and decode happen during preflight or preparation.

The source is placed by its layer's `start` and `duration`. `sizing` controls
intrinsic-to-canvas placement and `crop` controls the source crop. Layer
transform, opacity, blend mode, animation, effects, transitions, and image-only
presets are owned by the layer or its collections.

Image sources are supported by the canonical model, high-level Python, CPU,
and WGPU paths. A missing or unreadable asset is an environment/preflight
error. See [effects](../effects.md), [transitions](../transitions.md), and
[backends](../backends.md).
