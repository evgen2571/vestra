# Phase 8 authoring conformance

Phase 8 is complete when the typed Python API covers the complete current
schema-version 1 project model. `Project.from_dict()` is a lower-level
construction path for that same model. It accepts existing canonical JSON,
low-level integration data, and generated project dictionaries. It is not an
escape hatch for capabilities that the current schema and native model lack.

| Capability | Current schema-v1? | Typed authoring | `Project.from_dict()` | Status |
| --- | ---: | ---: | ---: | --- |
| Project `schema_version`, `name`, and `metadata` | yes | yes | yes | complete |
| Output `path`, `width`, `height`, `frame_rate`, `background`, `quality`, `audio`, `duration_mode`, and `duration` | yes | yes | yes | complete |
| Image and audio assets | yes | yes | yes | complete |
| Image clips: source, start, duration, layer, visible, sizing, crop, transform, opacity, effects, blend mode, and preset | yes | yes | yes | complete |
| Solid-colour clips: source, start, duration, layer, visible, opacity, effects, and blend mode | yes | yes | yes | complete |
| Transform tracks: position, anchor, scale, and rotation degrees | yes | yes | yes | complete |
| Scalar, point, and crop tracks; base values and keyframes | yes | yes | yes | complete |
| `linear`, `hold`, `ease_in`, `ease_out`, `ease_in_out`, and `cubic_bezier` interpolation | yes | yes | yes | complete |
| Ordered clip effects: brightness, contrast, saturation, tint, gaussian blur, directional blur, zoom blur, glow, chromatic aberration, vignette, sharpen, color adjust, camera shake, and motion blur | yes | yes | yes | complete |
| Ordered global post-effects: brightness, contrast, saturation, tint, gaussian blur, directional blur, zoom blur, glow, chromatic aberration, vignette, sharpen, and color adjust | yes | yes | yes | complete |
| Presets: slow drift, zoom punch, impact, heavy impact, and focus reveal | yes | yes | yes | complete |
| Transitions: crossfade, zoom crossfade, flash cut, directional push, and zoom blur | yes | yes | yes | complete |
| Standalone flashes | yes | yes | yes | complete |
| One global audio track: asset, timeline start, trims, volume, fades, and mute | yes | yes | yes | complete |
| Video assets | no | no | no | future model work |
| Multiple audio tracks, mixing, or audio crossfades | no | no | no | future model work |
| Nested compositions | no | no | no | future model work |

No current schema-v1 field is raw-`Project.from_dict()`-only. The typed API
writes minimal schema-v1 dictionaries; native conversion may expand defaults on
first serialization.

IDs are builder-local and deterministic: image/audio assets share `asset`,
clips share `clip`, each clip effect chain and global post-effects have separate
`effect` scopes, and transitions and flashes each have their own scope. Failed
factory operations validate before allocation.

Python owns category, finite-number, representability, ownership, and local
shape errors. Native parsing owns schema conversion; native validation owns
timeline fitting, dependent visibility, keyframe range/order, semantic effect
limits, and preset intervals. Preflight owns media/backend capability; rendering
owns FFmpeg and output publication failures. Authoring deliberately does not
repair dependent structures after mutation.
