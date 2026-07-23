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

Solid-colour clips also reject image-only sizing, crop, and preset fields.
Coordinated transitions reference visible image clips only.

Optional fields are omitted when unused; JSON `null` is never a substitute for
omission. This includes optional metadata, audio, sizing, crop, transform, and
audio trim fields.

For image clips, the renderer first resolves the normalized crop in source
coordinates, then applies sizing (`original`, `fit`, `cover`, `scale`, or
`stretch`), then anchor, scale, rotation, and position. `fit` and `cover` use
the output canvas dimensions. Crop values are normalized rectangles inside the
source (`x`, `y`, `width`, and `height`); static non-full crops can be cached,
while animated or oversized crops fall back to direct source sampling.

Tracks use clip-local seconds. `base_value` applies before the first keyframe;
each keyframe's interpolation controls the segment ending at that keyframe;
and the final keyframe holds after its time. Keyframe times must be strictly
increasing and lie within the clip. Position is normalized to the canvas,
anchor is normalized to the unscaled source, scale is positive, and source
pixel centres are sampled bilinearly after crop and sizing are resolved.

Brightness adds a normalized RGB offset, contrast uses `1` as identity,
saturation uses `1` as identity, and tint blends toward its colour with an
amount in `0..=1`. Effects are applied in declared order and their parameters
may be animated. Values are clamped only after the combined affine operation.

Render results report the runtime `requested_render_backend` (`auto`, `cpu`, or
`wgpu`) and selected `render_backend` (`cpu` or `wgpu`) separately from
`encoder_backend: "ffmpeg"`. An automatic pre-render fallback records a
structured `backend_fallback`; successful WGPU renders also report adapter
metadata. Backend selection is intentionally not a project-format property.
Timings are measured in milliseconds:
project parsing, semantic validation, plan compilation, asset decoding, track
evaluation, frame rendering, encoder write/finalization, output publication,
and total.
Frame rendering is a single non-overlapping CPU compositor interval. Cache
metrics report requests, hits, misses, insertions, evictions, current and peak
bytes/entries, and oversized skips; declared, hidden, rendered, and zero-frame
clip counts are kept distinct.

Crossfades reference visible image clips and compile to generated opacity
tracks. A transition is an additional multiplicative opacity contribution to
each referenced clip; overlapping transitions combine multiplicatively with
the clip's own opacity and any other transition contributions. The renderer keeps decoded sources under configured resource limits and
uses a byte-budgeted LRU cache for static crop materializations. Crops that
cannot fit are sampled from the original source without allocation.

Default limits are: output up to 8192×8192, 216,000 frames and 7,200 seconds;
100,000,000 source pixels per image; 400 MiB decoded per asset and 1 GiB total;
64 active layers; 10,000 clips; 32 effects per clip; 1,000 keyframes per
track; and a 256 MiB crop-cache budget.

Reports expose declared/rendered/hidden clips, decoded-source and cache bytes,
cache request outcomes, active/evaluated layers, effect counts, and measured
render timings. Cache `current_*` values describe the final cache state;
`peak_*` values are high-water marks; evictions and oversized skips explain
why a requested crop was not retained. The WGPU backend is headless and uses
the same evaluated frames and decoded source bytes as CPU; it uploads full
decoded images once, uses persistent output/readback resources, and transfers
each completed RGBA frame back to CPU for FFmpeg. It does not implement
zero-copy or hardware encoding, windowed preview, or advanced GPU effects.

## Effects-ready v1

Each clip may contain an ordered `effects` array and a `blend_mode` of
`normal`, `add`, `screen`, `multiply`, or `overlay`. The CPU renderer draws a
clip into a reusable local surface, runs effects in declared order, then blends
the result into the frame. `visual.post_effects` runs after all layers, also in
declared order. A two-surface ping-pong buffer avoids an allocation for every
effect pass.

Available effects are `gaussian_blur`, `directional_blur`, `zoom_blur`, `glow`,
`chromatic_aberration`, `vignette`, `sharpen`, `color_adjust`,
`camera_shake`, and `motion_blur`, in addition to the original colour effects.
Blur radii are bounded at 32 pixels. `zoom_blur` samples along scaled rays from
its normalized anchor, with `inward`, `outward`, or centered exposure and 2 to
32 samples. Glow extracts highlights into premultiplied alpha, blurs the
tinted signal, and composites it without discarding glow alpha outside the
source bounds. Sharpen is an unsharp-mask approximation. Colour
adjustment applies exposure, black and white levels, then gamma, preserving
alpha. Vignette uses aspect-correct canvas distance.

Camera shake is a continuous seeded timeline signal. `start` and optional
`duration` create a half-open clip-local active interval; outside it the shake
is identity and its attack and decay use effect-local time. Motion blur samples neighbouring timeline
transforms, derives translation direction in screen space, and caps the
directional blur. v1 does not derive blur from rotation or scale velocity.

Advanced effects, non-normal blend modes, and global post effects run on CPU.
`auto` chooses CPU with a structured capability fallback; explicit `wgpu`
rejects these projects before frame rendering. This avoids silent degradation.

Supported coordinated transitions are `crossfade`, `zoom_crossfade`,
`flash_cut`, `directional_push`, and `zoom_blur`. They must fit in both clips.
The compiler adds linked opacity, transform, flash, and blur tracks for the
participating layers.

`preset` expands during compilation into generated transform contributions and
effect tracks. The renderer evaluates authored transforms first, then applies
preset contributions, transition contributions, and camera shake. Position and
rotation contributions add to authored values; scale contributions multiply.
Generated transform contributions are identity outside their active interval,
so they never replace authored keyframes.
Every preset accepts clip-local `start` and `duration`. Omitted durations use
the remaining clip for `slow_drift`, 0.35 seconds for `zoom_punch`, 0.28 for
`impact`, 0.4 for `heavy_impact`, and 0.8 for `focus_reveal`, capped by the
remaining clip. Available values are `slow_drift`, `zoom_punch`, `impact`, `heavy_impact`, and
`focus_reveal`; every preset has an intensity in `0..=2`, and impact presets
need a stable `seed`. User effects run after generated preset effects. See
[`effects-ready-v1.json`](../../examples/projects/effects-ready-v1.json) for a
combined runnable project.

## Backend support

| Feature | CPU | WGPU | Auto |
| --- | --- | --- | --- |
| Basic transforms and colour effects | yes | yes | prefers WGPU when available |
| Advanced effects and blend modes | yes | no | falls back to CPU with a diagnostic |
| Global post-effects | yes | no | falls back to CPU with a diagnostic |
| Advanced transitions and presets | yes | no | falls back to CPU with a diagnostic |

Advanced image processing uses straight-alpha storage with premultiplied-alpha
accumulation during blur passes. Gaussian blur is separable and bounded to a
32-pixel radius. Motion blur derives translation only; scale and rotation do
not contribute to its velocity.

## Runnable examples

Focused fixtures live under [`examples/effects`](../../examples/effects),
[`examples/transitions`](../../examples/transitions), and
[`examples/presets`](../../examples/presets), and
[`examples/compositing`](../../examples/compositing). The schema check
discovers every JSON example and rejects focused previews shorter than 90
frames. Render the complete CPU preview set with
`./scripts/render-effect-examples.sh`; use `--skip-existing` only when a
previous render is intentional. Outputs are written under `examples/output`.
