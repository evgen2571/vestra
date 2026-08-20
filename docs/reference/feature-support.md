# Feature support

This matrix summarizes current implementation evidence. `supported` means
there is a public path and renderer/test evidence. `limited` means the feature
has explicit constraints. `not verified` is deliberately not a support claim.

| Feature | High-level Python | Canonical JSON | CPU | WGPU |
| --- | --- | --- | --- | --- |
| Image | supported | `image` | supported | supported |
| Video | supported | `video` | supported with media preflight | supported with media preflight |
| Solid color | `Color`/`SolidColor` | `solid_color` | supported | supported |
| Shape | supported | `shape` | supported | supported |
| Text | supported with a font path | `text` | supported with font availability | supported with font availability |
| Spectrum2D | supported through authoring/source wrappers | `spectrum2d` | supported with audio analysis | supported where the WGPU source path is available |
| Particle system | supported | `particle_system` | supported | supported where the WGPU source path is available |
| Nested composition | `CompositionLayer` | `group` | supported | supported |
| Visual effects | supported catalog | effect descriptors | descriptor-specific | descriptor-specific |
| Transitions | supported catalog | transition placements | supported | supported where the descriptor path is implemented |
| Audio clips | supported | `audio` | supported with media preflight | audio is prepared alongside the selected render path |

CPU and WGPU dispatch are not enough by themselves to prove identical behavior.
The exact source, effect, transition, audio, and backend pages state constraints
where the current tests do not establish parity.
