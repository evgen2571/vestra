# Project format

This page documents the current canonical JSON project format. The checked-in
[JSON Schema](../../schemas/project.schema.json) describes structure. Rust
deserialization and semantic validation impose additional rules.

## Version and top-level object

`schema_version` is `3`. The loader rejects another version with
`VESTRA-SCHEMA-VERSION`. Objects reject unknown fields. `Project.from_json` and
`Project.from_value` parse JSON only. They do not read assets or probe media.

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `schema_version` | integer | yes | Must be `3`. |
| `name` | string or null | no | Optional project name. |
| `metadata` | JSON value or null | no | User metadata retained by the model. |
| `output` | object | yes | Canvas, frame rate, encoding, duration, and audio policy. |
| `assets` | array | yes | Named image, video, audio, or font assets. |
| `visual` | object | yes | Visual clips, flashes, and transitions. |
| `audio` | object or omitted | no | Audio tracks and master effects. |

## Output

`output` contains `path`, positive `width` and `height`, `frame_rate`,
`background`, `quality`, `audio`, and `duration_mode`. `quality` is
`preview`, `balanced`, or `high`. `duration_mode` is `automatic` or
`explicit`; explicit duration requires `duration` and automatic duration
derives the usable timeline. Frame rates may be a positive number or a reduced
`N/D` string, within the implementation's supported range.

## Assets and visual content

An asset is `{ "id", "type", "source" }`, where `type` is `image`, `video`,
`audio`, or `font`. `source` is a path or media identity interpreted relative
to the project base directory during preflight.

Visual clips use a tagged `source` object. Current source tags are `image`,
`video`, `solid_color`, `shape`, `text`, `spectrum2d`, `particle_system`, and
`group`. A clip has an id, start, duration, layer, visibility and presentation
properties. Groups contain nested visual clips and represent canonical nested
composition content. `visual.flashes` and `visual.transitions` hold their
respective placement objects.

| Visual clip field | Type | Notes |
| --- | --- | --- |
| `id` | string | Unique in its owning visual group. |
| `source` | tagged object | See [Sources](sources/image.md). |
| `start`, `duration` | seconds | Project or owning-group local timeline; duration is positive. |
| `layer` | integer | Compositing order. |
| presentation | objects/properties | Opacity, transform, effects, and animation use their dedicated canonical forms. |

Groups hold nested visual clips with their own local timeline. Effects are
ordered `id`/`type` objects. Transitions connect sibling endpoints and use a
generic definition; signals appear in bindable scalar-property modifiers. See
[effects](effects.md), [transitions](transitions.md), and [signals](signals.md).

## Audio

An audio timeline has `effects` and `tracks`. A track has an id, `mute`, `gain`,
`effects`, and `clips`. A clip identifies an audio asset and has `start`,
`trim_start`, optional `trim_end`, `gain`, optional `gain_automation`, fades,
fade curves, mute, and effects. See [audio](audio.md) for the time domains and
effect catalog.

## Validation layers

JSON Schema checks shape, required fields, types, tagged variants, and many
ranges. Canonical semantic validation checks relationships such as unique
ids, timing, endpoint ownership, effect scopes, and cross-field constraints.
Environment preflight then resolves files, probes media, checks FFmpeg and
output readiness, and checks the requested renderer where applicable. A
schema-valid document can still fail semantic validation or preflight.

## Schema generation

`schemas/project.schema.json` is a hybrid checked-in artifact. Its base
definitions come from the schema template. `ve generate-schema` then updates
the Spectrum2D Nyquist bound and rebuilds visual and audio effect branches from
the Rust descriptor catalogs. The repository checks the generated result
against the checked-in schema.

## Compatibility

This page records the current public serialization contract. The project JSON,
Python and Rust APIs, CLI output, and diagnostic/report schemas are
compatibility-sensitive. No stronger semantic-version guarantee is asserted
here.
