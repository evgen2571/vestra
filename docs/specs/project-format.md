# Project format

`vestra` accepts one JSON project format, described by
[`schemas/project.schema.json`](../../schemas/project.schema.json). Project
objects carry a required `schema_version` field. Version `3` is the only
accepted version. Earlier versions are historical and rejected without migration.
Missing and unsupported versions fail during loading; the
editor does not guess a version or migrate project data.

## Audio timeline

The optional `audio` object is `{ "tracks": [...] }`. Tracks are mixer lanes
with a unique `id`, optional `mute` (default `false`), optional linear `gain`
(default `1.0`), and ordered `clips`. A clip has globally unique `id`, `asset`,
`start`, `trim_start`, optional `trim_end`, `mute`, `gain`, `fade_in`, and
`fade_out`. Omit optional `trim_end`; never serialize it as `null`.

```json
"audio": {
  "tracks": [{
    "id": "music",
    "gain": 0.8,
    "mute": false,
    "clips": [{
      "id": "intro",
      "asset": "song-a",
      "start": 0.0,
      "trim_start": 3.0,
      "trim_end": 8.0,
      "gain": 1.0,
      "fade_out": 1.0,
      "fade_out_curve": "equal_power"
    }]
  }]
}
```

Optional `gain_automation` is `{ "keyframes": [...] }`; each keyframe has
`time`, `gain`, and optional `interpolation` (`linear` by default, or `hold`).
Automation is clip-local after source trimming, must contain at least one point,
must start at `0`, and has strictly increasing finite nonnegative times and
finite nonnegative gains. Audio gain automation uses source-keyframe
interpolation: a keyframe's `interpolation` controls the segment from that
keyframe to the following keyframe. This differs from visual animation tracks,
where a keyframe's interpolation controls the segment ending at that keyframe.
The last audio gain keyframe's `interpolation` has no effect because it has no
following segment. Its gain holds through the selected clip end. For example,
this clip ramps from `0.0` to `1.0` over `0.0..0.5`, holds at `1.0` over
`0.5..1.0`, then holds `0.8` to clip end:

```json
"gain_automation": {
  "keyframes": [
    { "time": 0.0, "gain": 0.0, "interpolation": "linear" },
    { "time": 0.5, "gain": 1.0, "interpolation": "hold" },
    { "time": 1.0, "gain": 0.8 }
  ]
}
```

Execution rounds
each keyframe time to the nearest 48 kHz mixer sample, and successive keyframes
must resolve to strictly increasing samples. Keyframes that collapse to one
mixer sample are rejected during preflight. `fade_in_curve`
and `fade_out_curve` are `linear` by default or `equal_power`. Equal-power uses
`sin(pi*u/2)` for a fade-in and `cos(pi*u/2)` for a fade-out.
For paired equal-power envelopes over the same interval,
`gain_in^2 + gain_out^2 = 1`. The Python crossfade helper only configures these
ordinary clip fields over an existing overlap. It creates no separate canonical
crossfade node, moves no clip, and changes no trim.

Clips may overlap both within and across tracks. Tracks and clips preserve
declaration order. Linear gain is finite and non-negative: 0 is silence, 1 is
unity, and values above 1 amplify. Mute, zero gain, and `output.audio` do not
change validation or structural automatic duration. `output.audio` only makes
authored audio eligible for muxing.

When muxing is enabled, the FFmpeg backend opens each unique audible resolved
source path once, in first track-and-clip-use order. Repeated clips fan out
from that normalized 48 kHz stereo floating-point source with `asplit`, then
perform their own trim, gain, fades, and placement. It rounds all non-negative
logical seconds to the nearest 48 kHz sample, uses those samples for source
trim and timeline placement, then linearly sums clips within each track and
tracks at the master. FFmpeg mix normalization is disabled and the encoder
adds no limiter or peak normalization. The final stream is padded or trimmed
to the resolved project duration. Muted and zero-gain content remains part of
project validation and duration calculation but does not create an output
stream.

One render may open at most 128 unique audible resolved source paths. This is
an execution-resource limit, separate from the 4,096 authored audio-clip
limit and the 256 authored audio-track limit. The backend applies it after deduplication and before starting FFmpeg;
muted and zero-effective-gain branches do not consume the budget.
Projects may contain at most 16,384 authored audio gain keyframes, regardless
of mute or output-audio settings.

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

## Groups (V1)

A Group owns a recursive list of child clips. Child clip IDs are local to their
containing composition, and Group nesting is limited to 32 levels. Core
compilation and evaluation preserve this nesting: child scheduling and
animation use composition-local time while audio and other project-global
signals retain root project time. Nested clips use the same preset application
and compiler normalization path as root clips; active-layer limits apply
independently to each composition. Motion-blur shutter samples preserve the
corresponding root-project-time offset for global signals. CPU Group rendering
is supported: children compose into a transparent inherited-size intermediate
surface, the Group transform applies to the completed composition, effects use
ordinary layer effect semantics, and Group opacity and blend apply once at the
parent level. Nested Groups are supported on CPU. A Group is classified static
only when active descendants, their content, and the Group's own presentation
are invariant throughout its effective visible interval; otherwise it is
dynamic. Static Groups may use the CPU whole-layer cache at the same stage as
other cached layers (after Group transform/effects and before parent opacity or
blend). Cache keys are compiler-owned identities and cache lifetime is bounded
by the renderer worker budget; active transitions bypass this static proof.
Resource accounting clips nested activity to inherited ancestor/root visibility
windows without rewriting authored timing. Compilation source, layer, effect,
and keyframe totals include nested Groups; root-authored clip counts remain
root-only.

Root and nested Group transitions accept the same generic placements and resolve
endpoint IDs within their owning composition. A nested child ID cannot be used
as a root endpoint. Nested flashes and post-effects remain outside Group V1.
CPU and WGPU render supported Groups through isolated transparent
composition targets; WGPU temporary targets scale with nested depth and its
Group cache remains deliberately conservative. Python Group authoring is
supported through `ProjectBuilder.add_group_clip`; it preserves the owned
recursive tree and composition-local child timing. Group V1 does not provide
named reusable compositions, an independent canvas size, crop, sizing, or
preset fields; nested flashes and nested post-effects remain unsupported.

Solid-colour clips and flashes cover the full output canvas. They support
opacity and colour effects; transforms are intentionally not accepted for
solid-colour sources. A flash with no fade-out keeps its configured opacity for
its whole half-open interval. With a fade-out it holds until `end - fade_out`
and then reaches zero at `end`.

Solid-colour clips also reject image-only sizing, crop, and preset fields.
Coordinated transitions use generic schema-v3 placements. They reference
visible sibling Image or Group clips, or an endpoint supported through the
high-level capability adapter, within their owning composition.

Spectrum2D clips are presentation-only bars driven by authored Master audio.
Linear layouts support bottom, top, and center anchors with forward, reverse, or
center-out mapping. Center-out duplicates visual placement but not analysis
bands (24 analysis bands produce 48 displayed bars). Radial layouts use
`0°=up`, `90°=right`, `180°=down`, `270°=left`, clockwise-positive angles,
finite start-angle normalization, inner radius, partial or full sweep, and
outward, inward, or both growth. Solid colour, minimum bar height, fixed
along-bar gradients, and analysis-index-based across-band gradients are
supported. Their frequency bands are logarithmically spaced and smoothed by
attack/release durations; ordinary clip opacity, blend mode, and visual effects
apply. A project containing Spectrum2D requires authored Master audio and
reports `MVP-SPECTRUM2D-MASTER-AUDIO` when it is absent.

The typed Python authoring API also provides eight authoring-time presets:
`classic` (balanced bottom bars), `dense` (more bands and tighter spacing),
`neon` (responsive bars with Glow and Bloom), `mirror` (center-anchored bars),
`center_out` (mirrored frequency placement), `circle` (radial circle),
`neon_circle` (radial cyan-to-magenta bars with Glow and Bloom), and `arc`
(radial upper semicircle). Presets expand immediately into the normal
Spectrum2D source fields and effect collection; canonical JSON contains no
preset identity. For example, explicit arguments override preset values:

```python
spectrum = builder.add_spectrum2d_clip(
    start=0, duration=3, layer=1, preset="dense", colour="#ff00ff", height=0.30,
)
```

Manual Spectrum2D parameters remain supported and are the canonical JSON form.

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

Ordinary clip-effect tracks are evaluated in seconds relative to the start of
their clip. Camera shake has an active interval, and its parameter tracks use
seconds relative to that interval's start. Global post-effect tracks are
evaluated in project-relative seconds. These time domains are deliberate: a
post effect has no owning clip, while a camera-shake envelope starts when its
active interval starts.

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
zero-copy or hardware encoding, windowed preview, or asynchronous readback.
Current visual effects and blend modes execute through the same evaluated
effect-pass ordering as CPU.

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

All valid schema-version 3 effects, blend modes, post-effects, generic
transitions, and presets have WGPU plan mappings. `auto` can select CPU when no compatible
adapter is available or WGPU preparation fails. Explicit `wgpu` reports adapter,
device, resource, shader, or runtime failures. Once preparation selects WGPU,
the renderer does not switch backends mid-operation.

Supported coordinated transitions are generic placements whose Python built-in
definitions include `Crossfade`, directional pushes, zoom crossfades,
blur-backed crossfades, zoom blur transitions, and whip pans. They must fit in
both endpoints. Custom definitions use the same generic channels and ordinary
transition-local effects; preset identity is not serialized.

`preset` expands during compilation into generated transform contributions and
effect tracks. The renderer evaluates authored transforms first, then applies
preset contributions, transition contributions, and camera shake. Position and
rotation contributions add to authored values; scale contributions multiply.
Generated transform contributions are identity outside their active interval,
so they never replace authored keyframes.
Every image preset accepts clip-local `start` and `duration`. Omitted durations use
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
| Basic transforms and colour effects | yes | yes | prefers WGPU when preparation succeeds |
| Advanced effects and blend modes | yes | yes | falls back before rendering only when WGPU preparation fails |
| Global post-effects | yes | yes | falls back before rendering only when WGPU preparation fails |
| Advanced transitions and presets | yes | yes | falls back before rendering only when WGPU preparation fails |

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

| Config group | Output | Duration / frames | Resolution / rate | Demonstrates |
| --- | --- | --- | --- | --- |
| `effects/{camera-shake,chromatic-aberration,color-adjust,directional-blur,gaussian-blur,glow,motion-blur,sharpen,zoom-blur}.json` | `effects-*.mp4` | 5 s / 150 | 320×180, 30 fps (zoom blur: 360×640) | baseline, animated effect, recovery |
| `effects/vignette.json` | `effects-vignette.mp4` | 5 s / 150 | 360×640, 30 fps | 9:16 normalized vignette |
| `transitions/{directional-push,zoom-crossfade}.json` | `transitions-*.mp4` | 5 s / 150 | 320×180, 30 fps | stable outgoing and incoming footage around the transition |
| `presets/{focus-reveal,heavy-impact,impact,zoom-punch}.json` | `presets-*.mp4` | 5 s / 150 | 320×180, 30 fps | a timed transient followed by a settled source |
| `presets/slow-drift.json` | `presets-slow-drift.mp4` | 6 s / 180 | 320×180, 30 fps | full-duration drift |
| `compositing/{blend-modes,global-post-effects}.json` | `compositing-*.mp4` | 5 s / 150 | 320×180, 30 fps | layer blending and ordered global finishing |
| `projects/animation-effects.json` | `projects-animation-effects.mp4` | 6 s / 144 | 320×180, 24 fps | timeline animation, transition, flash, and effects |
| `projects/effects-ready-v1.json` | `projects-effects-ready-v1.mp4` | 6 s / 180 | 720×1280, 30 fps | realistic combined CPU effects-ready project |
