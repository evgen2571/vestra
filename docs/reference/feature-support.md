# Feature support

`supported` means a public path plus implementation/test evidence.
`not fully verified` is not a parity claim. Media preflight is an operational
requirement, rather than reduced renderer support.

`direct` means the canonical clip can carry ordinary presentation and be a
transition endpoint. `adapter-mediated` means high-level Python supplies that
presentation; the canonical source cannot carry it directly.

## Sources and group canvases

| Feature | Python and canonical JSON | Presentation | CPU | WGPU |
| --- | --- | --- | --- | --- |
| Image | `Image` → `image` | direct | supported | supported |
| Video | `Video` → `video` | direct | supported, requires media preflight | supported, requires media preflight |
| Solid color | `Color`/`SolidColor` → `solid_color` | adapter-mediated | supported | supported |
| Shape (including Line stroke masks) | Shape constructors → `shape` | direct | supported | supported |
| Text | `Text` → `text`; bundled, installed or file fonts | direct | supported, requires font preparation | supported, requires font preparation |
| Spectrum2D | `Spectrum2D` → `spectrum2d` | adapter-mediated | supported, requires prepared audio analysis | not fully verified |
| Particle system | `ParticleSystem` → `particle_system` | adapter-mediated | supported | not fully verified |
| Nested composition | `Group` or `CompositionLayer` → `group` | direct | supported | supported |
| Local group canvas | `Group(size=(width, height))` → optional group `size` | clipped local canvas; unsized inherits parent | supported | supported |

See the [source references](catalog.md#source-types) and
[nested compositions](nested-compositions.md) for exact construction and timing.

## Masks and track mattes

All supported forms below use schema-v1 `masks` or `matte` fields. See
[masks](masks.md) for ownership, coverage, coordinates and timing.

| Feature | Python / presentation contract | CPU | WGPU |
| --- | --- | --- | --- |
| Geometric layer masks | `layer.masks`; ordinary layer presentation | supported | supported |
| Image alpha masks | `layer.masks` + `MaskCoverageMode.ALPHA`; ordinary layer presentation | supported | supported |
| Image luma masks | `layer.masks` + `MaskCoverageMode.LUMA`; ordinary layer presentation | supported | supported |
| Text alpha masks | `layer.masks` + `Text`; owned source presentation | supported | supported |
| Video alpha/luma masks | `layer.masks` + `Video`; owner-local source timing | supported | supported |
| Spectrum2D masks | `layer.masks` + `Spectrum2D`; audio-reactive owned source | supported | supported |
| Particle system masks | `layer.masks` + `ParticleSystem`; simulation-local source timing | supported | supported |
| Group masks | owned `Group` source; nested owned composition | supported | supported |
| Soft feather on masks | `mask.feather`; layer-local, output-pixel units | supported | supported |
| Dynamic mask properties | keyframes, modifiers, signals; layer-local transform and coverage | supported | supported |
| Track Matte Alpha/Luma | `layer.set_matte(..., mode=MatteMode.*)`; same immediate composition; referenced layer coverage | supported | supported |
| Track Matte invert and chains | `invert=True`, acyclic references; dependency-ordered isolated presentation | supported | supported |
| Shared Track Matte source | multiple consumers may reference one layer; CPU frame reuse; static WGPU cache reuse | supported | supported |
| Image crop/sizing as a mask | rejected in mask context; use `mask.transform` instead | unsupported in v1 | unsupported in v1 |
| Composition-space masks | not exposed; not supported | unsupported | unsupported |

## Effects, transitions and audio

| Feature | CPU | WGPU |
| --- | --- | --- |
| Visual effects catalog | Supported | Not fully verified per effect. |
| Motion Tile | Supported | Supported; hardware evidence below. |
| Directional Blur, Radial Blur, Chromatic Aberration | Supported | Supported. |
| Palette mapping, ordered dithering | Supported | Supported on the adapters listed below. |
| ASCII / pseudo-ASCII | Supported | Supported on the adapters listed below. |
| Halftone, bounded pixel sorting, CRT styling | Supported | Supported on the adapters listed below. |
| Transitions | Supported | Not fully verified visually. |
| Audio clips | Shared media execution; renderer status does not apply. | Shared media execution; renderer status does not apply. |

See [effects](effects.md), [transitions](transitions.md) and [audio](audio.md)
for constructor, canonical and preparation contracts. Transitions support direct
endpoints and adapter-mediated Python endpoints. Audio clips support Python
authoring and canonical `audio` timelines.

## Effect presentation scope

Exact supported controls, defaults and ranges are in the
[effect parameter reference](effect-parameters.md).

| Feature | Python → canonical type | Presentation scope |
| --- | --- | --- |
| Motion Tile | `MotionTile` → `motion_tile` | Clip-only, pre-transform source sampling; image/video/shape/text/group sources. |
| Directional Blur | `DirectionalBlur` → `directional_blur` | Post-transform. |
| Radial Blur | `RadialBlur` → `radial_blur` | Post-transform. |
| Chromatic Aberration | `ChromaticAberration` → `chromatic_aberration` | Post-transform. |
| Palette mapping | `PaletteMap` → `palette_map` | Layer post-transform / global post-composition; optional local-time color loop. |
| Ordered dithering | `OrderedDither` → `ordered_dither` | Layer post-transform / global post-composition; fixed output-pixel pattern. |
| ASCII / pseudo-ASCII | `Ascii`, `PseudoAscii` → `ascii` | Clip/global, fixed composition grid; area analysis and prepared character-font or geometric glyph coverage. |
| Halftone | `Halftone` → `halftone` | Layer/group post-transform and global; rotated area-analysis cells. |
| Bounded pixel sorting | `PixelSort` → `pixel_sort` | Layer/group post-transform and global; sharp stable segmented runs. |
| CRT styling | `Crt` → `crt` | Layer/group post-transform and global; inverse-sampled alpha. |

## Verification detail and limits

CPU and WGPU dispatch alone do not prove identical behavior. The source, effect,
transition, audio and backend pages state constraints where tests do not
establish parity.

| Feature | Focused CPU evidence | WGPU evidence |
| --- | --- | --- |
| Motion Tile | supported | supported on hardware WGPU via GL/D3D12; software adapters are not hardware evidence |
| Palette mapping | supported; literal tone/filter/alpha and legacy regressions | verified on software Vulkan and hardware GL/D3D12 NVIDIA; exact focused frame parity, 1080p/4K resource checks |
| Ordered dithering | supported; literal tone/filter/alpha and legacy regressions | verified on software Vulkan and hardware GL/D3D12 NVIDIA; exact focused frame parity, 1080p/4K resource checks |
| ASCII / pseudo-ASCII | supported by focused alpha/partial/tiny-cell tests | verified on software Vulkan and hardware GL/D3D12 NVIDIA for random-access periodic colors and tonal threshold fixtures |
| Halftone | supported, focused visual/alpha tests | verified on software Vulkan and hardware GL/D3D12 NVIDIA; temporal/alpha parity and 1080p/4K checks |
| Bounded pixel sorting | supported, tie/threshold tests | verified on software Vulkan and hardware GL/D3D12 NVIDIA; temporal/alpha parity and 1080p/4K checks |
| CRT styling | supported, scanline/phase tests | verified on software Vulkan and hardware GL/D3D12 NVIDIA; temporal/alpha parity and 1080p/4K checks |

Stylization release examples include [a generated moving portrait](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/stylized-effects/README.md#fine-detail-reference-and-ascii-diagnosis)
with three palette-independent dither looks and zero-source-mix ASCII checks.
Verification uses the actual adapters named above; native Windows D3D12/Vulkan
is unverified. Optional new ASCII calibration, halftone shapes, sorting metrics
and CRT mask modes are deferred and are not exposed as supported controls.
