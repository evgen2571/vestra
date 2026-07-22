# Project format

`video-editor` accepts one JSON project format, described by
[`schemas/project.schema.json`](../../schemas/project.schema.json). Project
objects do not carry a format or project-version field.

Every image clip has a typed source, local timeline interval, layer, transform
tracks, opacity track, and optional ordered colour effects. A track has a
`base_value` and strictly increasing keyframes. Keyframes are clip-local and
use `linear`, `hold`, `ease_in`, `ease_out`, `ease_in_out`, or cubic Bézier
timing. Position is normalized to the output canvas; anchor is normalized to
the unscaled source. Positive rotation is clockwise because image coordinates
increase downwards.

Image transforms are sampled with subpixel bilinear affine sampling. The
inverse affine matrix is prepared once per evaluated layer and scanlines
increment source coordinates directly. Brightness, contrast, saturation, and
tint remain ordered in project data, but are combined into one affine RGB
operation for each evaluated layer.

Solid-colour clips and flashes cover the full output canvas. They support
opacity and colour effects; transforms are intentionally not accepted for
solid-colour sources. A flash with no fade-out keeps its configured opacity for
its whole half-open interval. With a fade-out it holds until `end - fade_out`
and then reaches zero at `end`.

Crossfades reference visible image clips and compile to generated opacity
tracks. The renderer keeps decoded sources under configured resource limits and
uses a byte-budgeted LRU cache for static crop materializations. Crops that
cannot fit are sampled from the original source without allocation.

Reports expose declared/rendered/hidden clips, decoded-source and cache bytes,
cache request outcomes, active/evaluated layers, effect counts, and measured
render timings. Some CPU-only limitations remain: there is no GPU backend,
shape source, or encoded-byte golden comparison.
