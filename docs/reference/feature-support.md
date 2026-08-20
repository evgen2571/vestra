# Feature support

This matrix summarizes current implementation evidence. `supported` means a
public path plus implementation/test evidence. `not fully verified` is
deliberately not a parity claim. Media preflight is an operational requirement,
not reduced renderer support.

| Feature | Python authoring | Canonical JSON | Presentation and transition endpoint | CPU | WGPU |
| --- | --- | --- | --- | --- | --- |
| Image | supported | `image` | direct | supported | supported |
| Video | supported | `video` | direct | supported, requires media preflight | supported, requires media preflight |
| Solid color | `Color`/`SolidColor` | `solid_color` | adapter-mediated | supported | supported |
| Shape | supported | `shape` | direct | supported | supported |
| Text | supported, requires a font path | `text` | direct | supported, requires font preparation | supported, requires font preparation |
| Spectrum2D | supported through authoring/source wrappers | `spectrum2d` | adapter-mediated | supported, requires prepared audio analysis | not fully verified |
| Particle system | supported | `particle_system` | adapter-mediated | supported | not fully verified |
| Nested composition | `CompositionLayer` | `group` | direct | supported | supported |
| Visual effects | supported catalog | effect descriptors | not applicable | supported | not fully verified per effect |
| Transitions | supported catalog | transition placements | direct endpoints and adapter-mediated Python endpoints | supported | not fully verified visually |
| Audio clips | supported | `audio` | not applicable | not applicable, media execution is shared | not applicable, media execution is shared |

CPU and WGPU dispatch are not enough by themselves to prove identical behavior.
The exact source, effect, transition, audio, and backend pages state constraints
where the current tests do not establish parity.

`direct` means the canonical clip itself can carry ordinary presentation and be
a transition endpoint. `adapter-mediated` means the high-level Python API can
provide that presentation, while the canonical source cannot carry it directly.
