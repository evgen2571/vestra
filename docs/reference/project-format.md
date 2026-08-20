# Project format

This is the canonical schema-v4 JSON format. The checked-in [JSON
Schema](../../schemas/project.schema.json) defines the complete structural
shape. Rust semantic validation adds current engine invariants, and preflight
checks assets, media, tools, output paths, and requested backends. Objects
reject unknown fields. Parsing with `Project.from_json` or `from_value` does
not read files or probe media.

## Project and output

| Field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | Current format is `4`; Vestra accepts valid schema-`3` projects and normalizes them to `4`. |
| `name` | string | no | Omitted when absent. |
| `metadata` | non-null JSON value | no | Omitted when absent. |
| `output` | object | yes | Output contract below. |
| `assets` | array of asset | yes | May be empty. |
| `visual` | object | yes | Root visual composition. |
| `audio` | object | no | Audio timeline; omitted when absent. |

| Field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `path` | string | yes | Requested output path. It must contain a non-whitespace character and end in `.mp4`, case-insensitively. |
| `width`, `height` | integer | yes | Canvas pixels. Each must be even and in `2..=8192`. |
| `frame_rate` | positive finite number or `"N/D"` | yes | Frames per second. `N` and `D` are positive decimal integers. Vestra reduces the rate and requires a numerator no greater than `240000` and a denominator no greater than `1000000`. |
| `background` | color | yes | `#RRGGBB` or `#RRGGBBAA`. |
| `quality` | enum | yes | `preview`, `balanced`, or `high`. |
| `audio` | boolean | yes | Output audio policy. |
| `duration_mode` | enum | yes | `automatic` or `explicit`. |
| `duration` | positive number | conditional | Seconds. Required by semantic validation for `explicit`; absent for `automatic`. |

## Assets and visual composition

| Field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `id` | non-empty string | yes | Unique within the asset collection. |
| `type` | enum | yes | `image`, `video`, `audio`, or `font`. |
| `source` | non-empty string | yes | Resolved relative to the project base directory during preflight. |

`visual` is the root composition. Its `clips` array is required; optional
`transitions`, `flashes`, and `post_effects` default to empty arrays. See
[transitions](transitions.md) and [effects](effects.md) for their dedicated
contracts.

| Clip field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `id` | non-empty string | yes | Unique among sibling clips. |
| `source` | tagged source object | yes | Tags: `image`, `video`, `solid_color`, `shape`, `text`, `spectrum2d`, `particle_system`, `group`. See [Sources](sources/image.md). |
| `start` | non-negative number | yes | Owning composition's local seconds. |
| `duration` | positive number | yes | Owning composition's local seconds. |
| `layer` | integer | yes | Local stacking order. |
| `opacity` | scalar property | yes | Base value plus optional animation and signal modifiers. |
| `source_start` | non-negative number | no | `0`; source-media seconds. |
| `playback_rate` | positive number | no | `1`; source-time rate. |
| `visible` | boolean | no | `true`. |
| `sizing` | sizing object | no | Applies to image/video sources. |
| `crop` | crop track | no | Applies to image/video sources. |
| `transform` | transform object | conditional | Required for image sources. Optional for video, shape, text, and group sources; an absent optional transform has canvas presentation. Forbidden for solid-color, Spectrum2D, and particle-system sources. |
| `effects` | array of effect | no | Empty. See [effects](effects.md). |
| `masks` | array of geometric mask | no | Empty; ordered layer-owned coverage inputs. See [masks](masks.md). |
| `blend_mode` | enum | no | `normal`; also `add`, `screen`, `multiply`, `overlay`. |
| `preset` | preset object | no | Omitted. See [presets and flashes](presets-and-flashes.md). |

## Timing, transform, groups, and bindings

Each mask has an `id`, a narrow shape `input`, `operation` (`replace`,
`intersect`, `union`, or `subtract`, default `intersect`), `invert` (default
`false`), `strength` (default `1`), `feather` (default `0`), and a mask-local
`transform`. These use the normal tracks, modifiers, and signal bindings.
Mask transforms are local to the owning layer and follow its presentation
transform. Only existing shape geometry inputs are supported in schema v4.

Track objects contain a required `base_value` and optional `keyframes`. A
keyframe has `time`, `value`, and `interpolation`; time is local seconds.
Interpolation is a named mode or a `cubic_bezier` object. Scalar properties
extend scalar tracks with ordered `modifiers`.

| Transform field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `position` | point track | yes | Finite canvas coordinates. |
| `anchor` | point track | yes | Normalized anchor position. |
| `scale` | point track | yes | Positive X/Y scale. |
| `rotation_degrees` | scalar property | no | `0`; finite degrees. |
| `component_modifiers` | object | no | Empty; optional `position_x`, `position_y`, `scale_x`, `scale_y` modifier arrays. |

| Group field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `type` | literal | yes | `group`. |
| `clips` | array of clip | yes | Child-local timeline and layer ordering. |
| `transitions` | array of transition placement | no | Empty. See [nested compositions](nested-compositions.md) and [transitions](transitions.md). |

| Binding field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `base_value` | number | yes | Authored scalar before animation/modifiers. |
| `keyframes` | array | no | Empty; each item has `time`, `value`, `interpolation`. |
| `modifiers` | array | no | Empty; applied in declaration order after authored animation. |
| `modifiers[].operation` | enum | yes | `replace`, `add`, or `multiply`. |
| `modifiers[].signal` | object | yes | `source.type` is `audio`, `source.tap` is `master`, and `source.feature` is `rms`, `peak`, or `band_energy`; optional `transforms` preserve order. See [signals](signals.md). |

## Audio

| Audio field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `tracks` | array of track | yes | May be empty. |
| `effects` | array of audio effect | no | Empty; master scope. See [audio](audio.md). |

| Track field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `id` | string | yes | Track identifier. |
| `clips` | array of audio clip | yes | May be empty. |
| `mute` | boolean | no | `false`. |
| `gain` | non-negative number | no | `1`. |
| `effects` | array of audio effect | no | Empty; track scope. See [audio](audio.md). |

| Audio clip field | Type | Required | Default / unit / notes |
| --- | --- | --- | --- |
| `id` | string | yes | Clip identifier. |
| `asset` | string | yes | References an audio asset. |
| `start` | non-negative number | yes | Project seconds. |
| `trim_start` | non-negative number | yes | Source-media seconds. |
| `trim_end` | non-negative number | no | Source-media seconds; semantic validation checks ordering. |
| `gain` | non-negative number | no | `1`. |
| `gain_automation` | object | no | Keyframes use clip-local seconds; see [audio](audio.md). |
| `fade_in`, `fade_out` | non-negative number | no | `0`; seconds. |
| `fade_in_curve`, `fade_out_curve` | enum | no | `linear`; `linear` or `equal_power`. |
| `mute` | boolean | no | `false`. |
| `effects` | array of audio effect | no | Empty; clip scope. See [audio](audio.md). |

## Validation layers

JSON Schema provides machine-readable structural validation: field names,
types, tags, and basic ranges. Canonical semantic validation checks cross-field
and engine invariants such as IDs, timing, endpoint ownership, effect scope,
and project relationships. Preflight then resolves assets and media, checks the
environment and output target, and verifies the requested backend. A
schema-valid project can fail either later layer.

`ve generate-schema` updates descriptor-derived effect branches and the
Spectrum2D Nyquist bound in `schemas/project.schema.json`. Repository checks
compare the generated output with that checked-in artifact.
