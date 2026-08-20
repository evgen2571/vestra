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
| Spectrum2D | supported through authoring/source wrappers | `spectrum2d` | supported with prepared audio analysis | source dispatch exists; full parity not verified |
| Particle system | supported | `particle_system` | supported | render path exists; full parity not verified |
| Nested composition | `CompositionLayer` | `group` | supported | supported |
| Visual effects | supported catalog | effect descriptors | catalog effect-pass path | catalog effect-pass path; full per-effect parity not verified |
| Transitions | supported catalog | transition placements | supported | supported; full visual parity not verified |
| Audio clips | supported | `audio` | supported with media preflight | audio is prepared alongside the selected render path |

CPU and WGPU dispatch are not enough by themselves to prove identical behavior.
The exact source, effect, transition, audio, and backend pages state constraints
where the current tests do not establish parity.
