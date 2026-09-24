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
| Shape (including Line stroke masks) | supported | `shape` | direct | supported | supported |
| Text | supported, requires a font path | `text` | direct | supported, requires font preparation | supported, requires font preparation |
| Spectrum2D | supported through authoring/source wrappers | `spectrum2d` | adapter-mediated | supported, requires prepared audio analysis | not fully verified |
| Particle system | supported | `particle_system` | adapter-mediated | supported | not fully verified |
| Nested composition | `CompositionLayer` | `group` | direct | supported | supported |
| Geometric layer masks | `layer.masks` | `masks` (schema v1) | ordinary layer presentation | supported | supported |
| Image alpha masks | `layer.masks` + `MaskCoverageMode.ALPHA` | `masks` (schema v1) | ordinary layer presentation | supported | supported |
| Image luma masks | `layer.masks` + `MaskCoverageMode.LUMA` | `masks` (schema v1) | ordinary layer presentation | supported | supported |
| Text alpha masks | `layer.masks` + `Text` | `masks` (schema v1) | owned source presentation | supported | supported |
| Video alpha/luma masks | `layer.masks` + `Video` | `masks` (schema v1) | owner-local source timing | supported | supported |
| Spectrum2D masks | `layer.masks` + `Spectrum2D` | `masks` (schema v1) | audio-reactive owned source | supported | supported |
| Particle system masks | `layer.masks` + `ParticleSystem` | `masks` (schema v1) | simulation-local source timing | supported | supported |
| Group masks | owned `Group` source | `masks` (schema v1) | nested owned composition | supported | supported |
| Soft feather on masks | `mask.feather` | `masks` (schema v1) | layer-local, output-pixel units | supported | supported |
| Dynamic mask properties | keyframes, modifiers, signals | `masks` (schema v1) | layer-local transform and coverage | supported | supported |
| Track Matte Alpha/Luma | `layer.set_matte(..., mode=MatteMode.*)` | `matte` (schema v1) | same immediate composition; referenced layer coverage | supported | supported |
| Track Matte invert and chains | `invert=True`, acyclic references | `matte` (schema v1) | dependency-ordered isolated presentation | supported | supported |
| Shared Track Matte source | multiple consumers may reference one layer | `matte` (schema v1) | CPU frame reuse; static WGPU cache reuse | supported | supported |
| Image crop/sizing as a mask | rejected in mask context | not serialized | use `mask.transform` instead | unsupported in v1 | unsupported in v1 |
| Composition-space masks | not exposed | not serialized | not supported | unsupported | unsupported |
| Visual effects | supported catalog | effect descriptors | not applicable | supported | not fully verified per effect |
| Motion Tile | `MotionTile`, dynamic scalar and point properties | `motion_tile` | clip-only, pre-transform source sampling; image/video/shape/text/group sources | supported | supported on hardware WGPU via GL/D3D12; software adapters are not hardware evidence |
| Directional Blur | `DirectionalBlur`, dynamic scalar properties | `directional_blur` | post-transform | supported | supported |
| Radial Blur | `RadialBlur`, dynamic scalar and point properties | `radial_blur` | post-transform | supported | supported |
| Chromatic Aberration | `ChromaticAberration`, dynamic scalar properties | `chromatic_aberration` | post-transform | supported | supported |
| Transitions | supported catalog | transition placements | direct endpoints and adapter-mediated Python endpoints | supported | not fully verified visually |
| Audio clips | supported | `audio` | not applicable | not applicable, media execution is shared | not applicable, media execution is shared |

CPU and WGPU dispatch are not enough by themselves to prove identical behavior.
The exact source, effect, transition, audio, and backend pages state constraints
where the current tests do not establish parity.

`direct` means the canonical clip itself can carry ordinary presentation and be
a transition endpoint. `adapter-mediated` means the high-level Python API can
provide that presentation, while the canonical source cannot carry it directly.
