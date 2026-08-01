# Phase 8 authoring conformance

The typed API writes minimal schema-v1 dictionaries. `Project.from_dict()` is
still the escape hatch for schema fields intentionally outside Phase 8; native
serde expands defaults on the first conversion.

| Schema-v1 feature | Status | Typed entry point / note |
| --- | --- | --- |
| project settings, output, metadata | fully typed | `ProjectBuilder` |
| image and audio assets | fully typed | `add_image_asset`, `add_audio_asset` |
| image and solid clips; timing, layer, visibility, opacity | fully typed | clip factories and properties |
| sizing, transform, crop, blend modes | fully typed | `Sizing`, tracks, `Crop`, `BlendMode` |
| animation, interpolation, cubic Bézier | fully typed | track `keyframe` methods |
| global audio | fully typed | one `set_audio`/`clear_audio` track |
| clip and global effects | fully typed | `clip.effects`, `builder.post_effects` |
| transitions and flashes | fully typed | builder collections |
| presets and timeline helpers | fully typed | image `presets`, `builder.timeline` |
| video assets, compositions, multiple tracks/mixing | intentionally deferred | outside Phase 8 |

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
