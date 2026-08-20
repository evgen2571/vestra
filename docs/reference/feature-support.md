# Feature support

This matrix summarizes current implementation evidence. `supported` means a
public path plus implementation/test evidence. `not fully verified` is
deliberately not a parity claim. Media preflight is an operational requirement,
not reduced renderer support.

| Feature | High-level Python | Canonical JSON | CPU | WGPU |
| --- | --- | --- | --- | --- |
| Image | supported | `image` | supported | supported |
| Video | supported | `video` | supported, requires media preflight | supported, requires media preflight |
| Solid color | `Color`/`SolidColor` | `solid_color` | supported | supported |
| Shape | supported | `shape` | supported | supported |
| Text | supported, requires a font path | `text` | supported, requires font preparation | supported, requires font preparation |
| Spectrum2D | supported through authoring/source wrappers | `spectrum2d` | supported, requires prepared audio analysis | not fully verified |
| Particle system | supported | `particle_system` | supported | not fully verified |
| Nested composition | `CompositionLayer` | `group` | supported | supported |
| Visual effects | supported catalog | effect descriptors | supported | not fully verified per effect |
| Transitions | supported catalog | transition placements | supported | not fully verified visually |
| Audio clips | supported | `audio` | not applicable, media execution is shared | not applicable, media execution is shared |

CPU and WGPU dispatch are not enough by themselves to prove identical behavior.
The exact source, effect, transition, audio, and backend pages state constraints
where the current tests do not establish parity.
