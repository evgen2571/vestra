# Stylized video effects

Status: in progress — initial effect families implemented; advanced artistic controls, visual regression, hardware quality and performance acceptance underway.
Branch: `feat/stylized-video-effects`
Baseline: `6ec571f6282456d0a595fd9a1aa3ca1f2359c092`
Initial decisions finalized: 2026-10-08; advanced-customization extension approved: 2026-10-09
Working instructions: [AGENTS.md](../../../AGENTS.md) and [PLANS.md](../../../PLANS.md)

## Objective

Deliver a general-purpose, composable suite of cinematic video stylization
effects in Vestra, with music-mix backgrounds as an example rather than a
special-case API. The branch includes initial implementations of **all** originally planned families: cinematic ASCII,
pseudo-ASCII/hybrid rendering, palette mapping and animated/rainbow color,
ordered dithering, halftone, bounded horizontal/vertical pixel sorting, and
CRT/analog styling. Reuse existing Bloom, Glow, ChromaticAberration and
ColorAdjust instead of duplicating them. The 2026-10-09 extension adds artist-facing
controls and regression acceptance before declaring the branch complete.

## Verified starting point and references

Existing infrastructure: canonical descriptor-driven visual effects and schema;
backend-neutral compilation, evaluation and ordered effect passes; CPU/WGPU
renderers; Python effect classes; keyframes and audio-reactive scalar signals;
licensed showcases and benchmarks.

Visual quality target for Milestone 2:
[detailed dithered-palette stylization](../../development/stylization/dithered-palette-look.md).
The goal is the reference's **fine pixel structure and tonal detail** with
arbitrary palettes, not a red-only effect or an additional effect family.

Read [effect extension](../../development/extending/effect.md),
[effect architecture](../../development/architecture/effect-pipeline.md),
[testing](../../development/testing.md),
[GPU validation](../../development/gpu-validation.md),
[performance](../../development/performance.md),
[effect reference](../../reference/effects.md),
[preset reference](../../reference/presets-and-flashes.md) and
[feature support](../../reference/feature-support.md).

Implementation entrypoints: `crates/vestra-core/src/effect_definition.rs`,
`crates/vestra-core/src/project/model/effects.rs`,
`crates/vestra-core/src/plan/effect_passes.rs`,
`crates/vestra-render/src/cpu/effects.rs`,
`crates/vestra-render/src/wgpu/`,
`crates/vestra-render/src/shaders/effects/`,
`python/vestra/effects/`, `python/vestra/presets.py`,
`examples/effects/` and `benchmarks/`.

## Locked product and architectural decisions

These decisions were explicitly approved for this branch. Do **not** turn them
back into questions or reduce scope without reporting a substantive blocker.

| # | Area | Required outcome |
| --- | --- | --- |
| 1 | Effect scope | Ship all planned effect families, not just ASCII |
| 2 | ASCII control | Advanced customization: glyph selection, character density/cell sizing, edge/fill modes, mixing with the source image and other meaningful controls |
| 3 | Color | Monochrome, sampled source colors, custom palettes/gradients, rainbow and animated palettes, and appropriate audio-reactive controls |
| 4 | Presets | Independent, composable effects plus optional curated cinematic presets/recipes; reuse the existing preset system only where its semantics fit |
| 5 | Loopability | Deterministic, explicitly loopable effect-generated animation |
| 6 | Renderers | **Both CPU and WGPU** must implement the same exposed effect families and public semantics |
| 7 | Performance | Optimize for **1080p**; ensure **4K support** within documented device/resource limits, without promising real-time rendering |
| 8 | Pixel sorting | Deterministic, bounded **horizontal and vertical** segmented sorting |
| 9 | Licensing | Open-source shader references/ports are allowed when their licenses permit; preserve required notices and maintain provenance for bundled assets |
| 10 | Placement | Clip-level and global post-effects wherever semantically applicable |
| 11 | Glyph assets | Provide built-in character sets **and** user-supplied character strings, fonts and glyph selection |
| 12 | Preset integration | Use existing presets when appropriate, with reusable effect combinations for other sources; avoid a competing preset framework |
| 13 | Animation period | Expose an **explicit loop period** for effects supporting procedural/periodic animation |

### Presets and source compatibility

Current cinematic presets are restricted to **Image layers** and at most one
per image layer (see [reference](../../reference/presets-and-flashes.md) and
`python/vestra/presets.py`). Do not imply they already support Video,
all layers or global output. Prefer a thin reusable composition/recipe of
ordinary ordered effects for stylization, including Video and global usage.
Reuse or extend existing preset infrastructure only if it is a clean fit;
do not create another independent renderer or incompatible preset catalog.
Individual effect parameters remain configurable when a preset is used.

### ASCII inputs and customization

Built-in character sets and a redistributable, documented default glyph
resource must work out of the box. Also support custom character sequences
and user-provided fonts (or an equivalent explicit custom-glyph resource
interface), through the portable public authoring model.

The implementation design must specify character ordering/selection, Unicode
coverage, unsupported/missing glyph handling, font shaping/rasterization scope,
fallback behavior, atlas dimensions, path resolution/preflight, caching, schema
representation and cross-backend asset reuse. Invalid/missing font resources
must fail predictably; no silent disappearance of unsupported glyphs.
Keep source files and licensing obligations distinct from rendered artifacts.

### Periodic animation semantics

Supported procedural parameters (such as rainbow phase, palette rotation,
and suitable CRT motion) expose a **positive finite period** in documented
timeline time units (seconds). The generated periodic component must be
deterministic at arbitrary evaluation times, including nonsequential frame
rendering, with mathematical continuity at the cycle boundary; do not base
phase on wall-clock time or prior frames. A period may be omitted/disabled
when an effect has no procedural animation.

Specify phase/time-origin, period validation, interaction with keyframes and
audio-signal modulation, and how static caching recognizes a timed effect.
An effect's own periodic component repeats every period; the final video
will only loop seamlessly if its **source footage, audio and any other
animations** also align. Do not claim automatic looping of arbitrary inputs.
Tests must render the same timestamps out of order and compare times separated
by exactly one period. Choose periodic functions that produce the same
boundary state without requiring duplicated final frames.

### CPU/WGPU, scope and performance policy

CPU and WGPU must expose the same user-facing controls and meaningful output,
with documented **numerical/perceptual tolerances**, not an unrealistic
bit-identical-across-devices requirement. Optimize and benchmark typical
1080p moving-video workloads; run 4K correctness/memory tests where the
adapter supports required resources and report device-limit failures clearly.
Offline rendering need not meet a real-time FPS target.

Keep new effects post-transform by default and preserve authored ordering;
support clip and global effects where their semantics apply. Test effects on
video, images, masks/mattes, nested compositions and transparent inputs.
Do not advertise GPU-only versions as complete CPU/WGPU parity.

### Temporal stability and visual-quality requirements

Build temporal stability **into each relevant effect** during Milestones 2–4,
rather than postponing it to final optimization. These requirements apply to
real moving video and deterministic arbitrary-time frame rendering. They do
not promise that every discontinuity in source content or intentional glitch
motion can be removed. Retain the documented effect-specific visual style:
stabilization must not blur deliberate fine dithering, hard palette steps or
pixel-sort glitches into an unrelated look.

| Technique | Implementation policy | Effects / milestone |
| --- | --- | --- |
| Stable spatial pattern anchoring | **Required.** Define and test pixel-center, grid-origin and transform behavior. Keep existing output-pixel Bayer anchoring for OrderedDither; use deliberate composition/layer/source-space contracts for other effects, not a universal screen-space assumption. Patterns must be deterministic for identical time and inputs. | Dither (2), ASCII (3), halftone/CRT (4) |
| Spatial prefiltering | **Required where cell/region analysis needs it.** Apply appropriately scaled anti-alias or filtered luminance/color analysis to avoid tracking single noisy samples. Make pixel-sort input filtering optional or avoid it when it would destroy intentional hard edges/glitches; justify any additional pass. | ASCII (3), halftone (4), pixel sorting if justified (4) |
| Area-averaged cell analysis | **Required.** Use representative alpha-aware area statistics for character and halftone cells (including partial border cells), rather than a single pixel sample. Define behavior for transparent inputs, small cells and boundaries. | ASCII (3), halftone (4) |
| Antialiased procedural geometry | **Required for generated geometric edges.** Reuse antialiased atlas coverage for font glyphs; smooth halftone-dot boundaries and appropriate CRT geometry analytically or with justified supersampling/filtered coverage. Keep intentionally hard pixel-art modes available. | ASCII (3), halftone/CRT (4) |
| Controlled threshold transitions | **Selective, required when thresholds visibly chatter.** Test small luminance/parameter changes near glyph, dot, dither and sort boundaries. Use appropriate stable sampling or tunable soft transitions/hysteresis-like *stateless* mapping when beneficial; keep hard/detailed variants and palette quantization exact where intended. No history-dependent selection. | Dither (2), ASCII (3), halftone/sorting (4) |
| Continuous animated-parameter evaluation | **Required for interpolable controls.** Keyframes, audio-modulated properties and procedural values must evaluate smoothly where specified, including nonsequential times and exact cycle boundaries. Discrete enums, glyph sets, matrix sizes and other incompatible choices remain explicitly discrete unless a transition strategy is implemented. | All applicable effects (2–4) |
| Deterministic temporally continuous noise | **Required for any animated stochastic-looking pattern.** Prefer seeded, continuous, project-time-based noise/functions that do not re-randomize each frame; fixed spatial patterns stay fixed by default. Audio modulation and explicit periods must not introduce hidden frame-state dependence. | CRT (4); animated dither variants only if introduced (2) |
| Stable bounded sort and segmentation | **Required.** Deterministic stable ties, bounded segments, consistent horizontal/vertical ordering, threshold eligibility and edge-block behavior. Small input changes may legitimately alter sort membership; tests must distinguish expected changes from nondeterministic reordering. | Pixel sorting (4) |
| Multi-resolution analysis | **High priority where quality warrants it.** Choose analysis scale relative to glyph/dot cell size; avoid aliasing from tiny textures and preserve important silhouettes at varying output sizes. Prefer reuse of existing prepared resources and bounded downsampling rather than unconditional new full-frame passes. | ASCII (3), halftone (4) |
| Temporal flicker diagnostics and regression | **Required validation.** Generate reproducible adjacent-frame and small controlled-input-change sequences plus per-frame difference/flicker metrics or diagnostic images using the [visual-regression skill](../../../.agents/skills/visual-regression/SKILL.md). Include stable-scene controls, moving subjects, threshold sweeps, effect-order comparisons and authored animation; distinguish expected scene motion/cuts and deliberate flicker from artifacts. | Start in 2, expand for 3–4, final acceptance in 5 |
| Effect/configuration transition quality | **High priority for useful transitions.** Reuse continuous `amount`/source blending and existing keyframe interpolation first. For incompatible discrete configurations (character set, dithering matrix, palette cardinality, dot pattern), evaluate an explicit transition/crossfade only if a real visual problem justifies the additional passes/resources; otherwise document that changes are discrete. Do not create a new general transition engine. | Reusable effects/looks in 4, demonstrations in 5 |

For temporal validation, capture **the same authored timestamps in different
render orders**, controlled stationary/moving fixtures, and short frame
sequences around quantization/glyph/segment thresholds and loop seams.
Report actual adapter and CPU/WGPU results; numerical temporal metrics are
diagnostics, not a universal flicker-free pass/fail threshold. Use subjective
review alongside metrics. Measure any prefilter, downsample or crossfade
overhead at 1080p, and test memory/resource bounds at 4K. Preserve explicit
effects' time-origin, caching and alpha contracts from the technical design.

### Licensing and references

[AcerolaFX](https://github.com/GarrettGunnell/AcerolaFX) is a licensed
reference, **not** an immediately executable WGSL shader. A port must adapt
to video-safe RGB/edge analysis because ordinary video has no game-scene
depth/normal textures. Check each file/license rather than assuming an
entire reference repository has homogeneous licensing. Preserve applicable
copyright/license notices when distributing source or derived components.

Document provenance, licenses and redistribution requirements for built-in
fonts/glyphs, sample footage and any shader adaptations. Follow
[example asset credits](../../../examples/showcase/ASSETS.md). A third-party
video is not freely reusable merely because effects change its appearance.

## Scope and non-goals

In scope: all named effect families, reusable parameter/API contracts,
CPU and WGPU functionality, custom fonts/glyphs, clip/global integration,
loopable procedural controls, optional preset/recipe conveniences,
real media showcases, tests, bounded resource use and public documentation.

Out of scope: a general shader-plugin language, rewriting the renderer,
procedural video-source framework, automatic arbitrary-video seamless looping,
audio-analysis overhaul or a special API coupled to one music-mix project.
The demo may repeat an existing video clip with existing timeline techniques.

## Implementation decisions delegated to the agent

The *product choices above are settled*. Determine and document implementation
details before freezing APIs: exact class/type names and parameter defaults,
ranges and units; atlas format/glyph coverage; font preflight and packaging;
spatial anchoring and edge treatment; luminance/color/alpha math; CPU/WGPU
pass topology and resource bounds; 1080p benchmark workloads; 4K limits; visual
test tolerances; and reasonable preset/recipe representation. Use current
descriptor, compiler and bounded texture-slot architecture unless a measured
or tested need justifies changes. Keep design notes in this plan.

## Milestones

### Technical contracts and resource map (2026-10-08)

All new effects use post-transform image coordinates, preserve authored order,
and support layers (including groups/video) and global post-effects. Working
RGB is encoded byte space, straight alpha. Tone selection uses integer
`54*R + 183*G + 19*B`, normalized by 65280; this approximates Rec.709
luminance without device-dependent threshold rounding. Fully transparent
pixels keep their input bytes for color-only operations and contribute no
color to neighborhood analysis. Color palettes contain 2–16 opaque colors
in authored dark-to-light order; colors never determine pattern geometry.

| Effect / tag | Public contract | Pass topology / resources |
| --- | --- | --- |
| `PaletteMap` / `palette_map` | Palette; `mode=gradient\|nearest\|rainbow`; bindable `amount` [0,1], `phase` in cycles; optional positive finite `period` in seconds | Single Current→Current pass; 16 packed RGBA colors in uniform record; no persistent/frame assets |
| `OrderedDither` / `ordered_dither` | Palette/color animation as above; bindable `strength` [0,1]; `matrix=bayer2\|bayer4\|bayer8`; integer `scale` [1,32] pixels; amount [0,1] | Single Current→Current pass; integer Bayer indexing anchored at (0,0), no random/time-varying threshold pattern |
| `Ascii` / `ascii` | Built-in/custom character sequences and optional Font asset; cell dimensions/density; luminance/edge/hybrid selection; mono/source/palette/rainbow coloring; glyph/background intensity and source mix; explicit palette period | Cell analysis Current→Temporary0; resolve OriginalAnd(Temporary0)→Current; retain original in existing auxiliary slot; prepared atlas binding, no readback/history |
| `Halftone` / `halftone` | Bindable cell size [2,64] px, finite screen angle, amount [0,1], softness [0,2] px; luminance/source/RGB screens; opaque foreground/background; invert | Rotated-cell analysis Current→Temporary0; resolve OriginalAnd(Temporary0)→Current; alpha-weighted exact cell sums, byte-quantized means and analytic dots |
| `PixelSort` / `pixel_sort` | Horizontal/vertical; ascending/descending; bindable lower/upper thresholds [0,1]; block length [2,256], amount [0,1] | Single pass; CPU stable bounded runs; GPU shared-memory segmented bitonic sort, kernel-specific block/line dispatch, ≤8 KiB shared storage |
| `Crt` / `crt` | Curvature, scanline/phosphor strengths and spacing, grain, jitter, flicker, rolling-band width/strength; bindable amount/phase; optional positive finite period and fixed seed | Single pass; alpha-aware inverse sampling; periodic sinusoidal/hash-coefficient motion; no history textures |

Palette defaults are black/white, amount=1, phase=0, period disabled;
PaletteMap defaults to gradient interpolation, OrderedDither to nearest,
strength=1, Bayer8, scale=1. Dithering selects adjacent authored tone levels
by `floor(tone*(N-1)+0.5+strength*(threshold-0.5))`, clamped to the palette.
Scale enlarges threshold cells without smoothing source detail. Matrix
thresholds are centered ranks `(rank+0.5)/(size*size)`; endpoints stay calm.
OrderedDither always selects discrete colors; mode controls palette generation,
with gradient/nearest both using the authored discrete palette in this effect.

Color animation is evaluated in core: effective phase is
`(authored_phase + owner_local_seconds/period) mod 1`, or authored phase alone
when period is absent. Custom palette motion continuously interpolates each
stop toward its cyclic successor; rainbow generates 16 HSV stops with hue
`phase+i/15`, saturation=1 and value=`i/15`. The core evaluates colors once
per effect/frame into RGBA bytes shared by both backends. Keyframes/signals
modulate authored phase, amount and strength before procedural phase is added.
Period itself is static, validated positive/finite, and marks the effect
dynamic even with constant source/properties. Nanosecond owner time determines
phase; random frame access does not change results. Bytes impose normal
1/255 color quantization, with continuous underlying cycle and equal boundaries.

Pixel-sort thresholds select contiguous eligible runs within fixed blocks;
alpha-zero/ineligible pixels break runs. Complete RGBA pixels move, integer
luminance ties preserve original position in both directions/orders. Final
partial blocks are bounded. Amount blends original/sorted in premultiplied
space. Authored lower>upper is invalid; evaluated crossing makes eligibility
empty. No unbounded full-row/full-column sort is implied.

Halftone dots use radius `sqrt(tone)*cell_size/sqrt(2)` to reach cell corners
at white, rather than leave dark texture in full highlights. Screen rotation
is anchored to canvas origin. RGB screens have fixed channel angle offsets.
CRT inverse curvature may create transparent borders; scanlines, masks/grain
do not create alpha. Periodic grain interpolates fixed seeded sine/cosine
coefficients, never hashes frame/time indices; jitter/flicker/rolling band use
periodic functions. Existing Bloom/Glow/ChromaticAberration/ColorAdjust remain
independent effects used in optional looks.

Recipes will be plain functions returning fresh ordered effects, compatible
with video/global/group stacks and editable after attachment. Existing image
presets remain image presets. No additional preset renderer/catalog is needed.

Validation fixtures: hand-derived ramps, all Bayer ranks, transparent/partial
alpha, thin silhouettes/textures, same scene with monochrome/cool/warm palettes,
nonsequential t and t+period, moving synthetic video, clip/global/mask/group
stacking. Color/dither target max channel error ≤1 before composition, ≤2
after composition, with zero pixels beyond the chosen tolerance. Spatial
threshold choices must match exactly. ASCII/halftone/CRT tolerances will be
fixed from actual rendered edges, not widened to conceal shader failures.
Benchmark prepared-frame and full moving-video costs at 1080p; verify 4K and
device limits separately. Use existing benchmark/adapter tooling.

ASCII asset contract: `font` is an optional Font asset ID (resolved by SDK
preflight against the project base directory); omission selects bundled DejaVu
Sans, reusing the repository's licensed test font. First font face only;
no system fallback. Characters are independent Unicode scalars, 1–256 entries
in authored dark-to-light order, with duplicates retained as intentional
weighting. No ligatures, grapheme/bidi/contextual shaping. Reject controls
except ordinary space, missing glyphs, and invisible non-space glyphs at
preflight/preparation with resource-specific diagnostics. `edge_characters`
contains exactly four directional scalars in horizontal/vertical/slash/backslash
order; coverage is checked against the chosen font as well.

The API uses `Ascii` (`ascii`) with `glyph_style=characters|geometric`:
the geometric option is the explicit pseudo-ASCII family, using density-ranked
dot/line/cross masks through the same analysis and resolve contract. Python may
expose `PseudoAscii` as a convenience selecting geometric style. Built-in
standard/dense/blocks sets expand to ordinary character strings in authoring.
Default characters are ` .:-=+*#%@`, edges `-|/\\`, cell_width=8 and
cell_height=12 pixels (bindable, rounded once by core; ranges 2–64 and 2–128),
mode=`hybrid` (`fill|edges|hybrid`), edge_threshold=.15 [0,1],
edge_strength=1 [0,4], invert=false, source_mix=0 [0,1], amount=1 [0,1].
Foreground coloring uses `color_mode=monochrome|source|palette|rainbow`,
foreground white, background opaque black, ordered palette black/white,
phase=0 cycles and optional positive finite period. Color-only phase is
evaluated through the same palette code as Milestone 2. Background may be
transparent; final glyph/background support is intersected with source alpha.
`source_mix` and amount mix in premultiplied space, preserving transparent
borders and hybrid footage detail.

Fixed atlas tiles are 32×48 pixels on 16 columns, with a shared baseline and
antialiased glyph coverage. Up to 256 fill +4 edge glyphs require 512×816
RGBA8 (≤1.6 MiB), prepared once in shared DecodedAssets and uploaded once to
WGPU. Resource keys include font/face, ordered characters and fixed raster
settings; cell-size animation never rebuilds fonts/atlases. Existing source
pixel, per-asset and aggregate byte limits apply before allocation; report
atlas bytes in preparation/backend counters. Preserve the font's complete
Bitstream/DejaVu license in `licenses/` and third-party notices.

Each cell's alpha-weighted RGB mean and selected glyph/edge information occupy
two RGBA8 metadata texels in Temporary0. Glyph indices use two bytes (maximum
259). Cells include only actual pixels at partial borders; luma/edge analysis
ignores hidden transparent RGB. Fill selection uses mean tone; edge selection
uses video-safe image gradients with deterministic orientation bins and ties,
never game depth/normal buffers. Resolve uses original pixel alpha and color,
cell metadata and prepared atlas. Add one prepared atlas sampled binding to
the effect layout (three sampled textures), a prepared resource index and
cell-grid dispatch extent. Existing Original/Temporary0/Current slots suffice;
there is no arbitrary render graph or per-frame font rasterization/readback.

Remaining defaults/ranges: Halftone cell_size=6, angle_degrees=15, softness=.5,
mode=luminance (`luminance|source|rgb`), foreground white/background black,
invert=false, amount=1. PixelSort direction=horizontal, order=ascending,
lower_threshold=.15, upper_threshold=.9, segment_length=64, amount=1.
CRT amount=1, curvature=.08 [0,.5], scanline_strength=.2 [0,1],
scanline_spacing=2 [1,8] pixels, mask_strength=.15 [0,1],
mask_spacing=1 integer [1,6] pixels, grain=.025 [0,.25], jitter=.35 [0,8]
pixels, flicker=.025 [0,.25], rolling_strength=.06 [0,1],
rolling_width=.12 [.01,1] height fraction, phase=0, period disabled and seed=0.
All continuous controls are bindable scalar properties; static modes, colors,
glyph strings/font, integer sort/mask limits and period remain authored values.
Periodic CRT controls use integer cycle counts and fixed seeded sinusoidal
coefficients; fractional scanline spacing changes spatial frequency without
changing the number of temporal cycles. Exact shader geometry/raster details
will be tested before each family's implementation is accepted.

### 1. Documentation and technical design

- [x] Add agent guidance, resumable plan convention and effect architecture page.
- [x] Expand effect extension guide and documentation navigation.
- [x] Record all 13 product decisions, including custom fonts, preset
  compatibility and explicit period semantics.
- [x] Specify and review effect API schemas/parameters, full effect inventory,
  algorithms, RGB/alpha/border contracts, glyph handling and visual fixtures.
- [x] Map each effect's CPU/WGPU pass/resource topology and identify justified
  extensions to prepared resources and renderer limits.

Completion: implementation-ready technical contracts without an unrelated
renderer rewrite.

### 2. Color and foundational primitives

- [ ] Reproduce the [palette-agnostic fine-detail dither target](../../development/stylization/dithered-palette-look.md),
  preserving outlines, midtone texture, shadow clarity and temporal stability
  with user-selected colors and adjustable fine/coarse pixel structure.
- [ ] Verify fixed output-pixel Bayer anchoring and deterministic threshold
  coverage on both backends; control edge/tonal threshold chatter without
  softening intentional crisp dither pixels or making patterns color-dependent.
- [ ] Verify continuous keyframed/audio-modulated/periodic palette evolution,
  including loop seams and random-access evaluation; introduce reproducible
  short-sequence temporal flicker diagnostics and a stable-scene control.
- [ ] Implement palette mapping, custom gradient/palette color modes and
  deterministic ordered dithering on both CPU/WGPU.
- [ ] Expose time-dependent palette/rainbow controls and explicit loop periods;
  support appropriate keyframe/signal binding via canonical evaluation.
- [ ] Extend intermediate-resource support only for concrete needs; verify
  resource limits and time-dependent cache correctness.
- [ ] Add API/schema/typing tests, deterministic frame fixtures, public docs
  and 1080p baseline measurements.

Completion: useful composable color/dither effects with verified periodic
behavior, public API and both backends.

### 3. ASCII and pseudo-ASCII

- [ ] Implement a bundled licensed glyph set, custom characters/fonts,
  validation/preflight and reusable glyph assets.
- [ ] Deliver density/cell controls, luminance and edge modes, monochrome,
  source-color and animated palette coloring, and hybrid source blending.
- [ ] Use alpha-aware **area-averaged cell statistics**, appropriate spatial
  prefiltering/multi-resolution sampling and antialiased glyph coverage.
  Specify intentional cell anchoring, partial-cell behavior and stable
  selection near glyph/edge thresholds without forcing blurred characters.
- [ ] Verify glyph/edge choices and spatial detail on static and moving
  footage across cell sizes, transforms and 1080p/4K resolutions, with
  controlled-input-change and short-sequence flicker diagnostics.
- [ ] Implement and verify equivalent CPU/WGPU paths, transparent/partial
  cell behavior, signal/keyframe support and applicable loop periods.
- [ ] Verify nonsequential frames, moving footage, typical 1080p throughput
  and 4K correctness/resources on available adapters.

Completion: advanced, deterministic cinematic ASCII usable on footage and
images, with no per-frame font/atlas rebuilds or GPU-to-CPU roundtrips.

### 4. Remaining stylization and reusable looks

- [ ] Implement halftone and CRT/analog styling on CPU/WGPU, including
  applicable periodic/animated parameters.
- [ ] For halftone, use representative alpha-aware **area-averaged** cell
  analysis, scale-appropriate spatial prefilter/multi-resolution sampling,
  stable lattice anchoring and antialiased procedural dot edges. Test tonal
  boundary behavior while preserving optional crisp graphic patterns.
- [ ] For CRT, anchor spatial scanlines/masks consistently, antialias generated
  geometry where appropriate, and use seeded **continuous project-time noise**
  for animated grain/jitter. Verify smooth modulation and repeatable periods.
- [ ] Verify stable segmented sorting with deterministic ties and threshold
  boundaries on moving inputs; keep intentional sharp glitches. Optional input
  prefiltering must not silently change the authored sort style.
- [ ] Implement **both** horizontal and vertical bounded segmented pixel
  sorting with explicit thresholds, stable ties and resource-limit behavior;
  optional exotic variants may be deferred, not either required direction.
- [ ] Supply a small set of optional curated looks using current presets
  where valid and composable ordered effect recipes elsewhere. No parallel
  preset engine.
- [ ] Animate continuous effect intensity/parameters through existing
  keyframes and blending. Assess configuration changes between discrete
  styles; offer explicit crossfading where worthwhile and affordable, or
  document their step-change behavior without adding a general transition
  subsystem.
- [ ] Verify order, global/clip scope, stacking, masks/mattes, color/alpha
  parity and error paths for each effect.

Completion: every agreed effect family exists with matching public behavior
and known resource/performance limits.

### Advanced artistic-control extension (approved 2026-10-09)

The technical contracts and default algorithms above describe the **existing
first implementation**, not the final intended feature set. Milestones 5–7
extend these contracts; new parameter names/ranges, schema changes and pass
topology require explicit design/validation before coding. Do not silently
change the behavior of projects authored against the existing defaults.

- **Composable first:** use existing `ColorAdjust`, contrast, bloom and
  ordered effect chains where their scope suffices; add effect-local controls
  only when they change *analysis* independently from final image appearance
  (for example, pre-quantization tone response or edge-preserving sampling).
  Reuse shared color/analysis helpers where worthwhile, without rewriting the
  renderer or introducing an independent preset engine.
- **Dither is not palette mapping:** retain the existing luminance-to-palette
  ordered-dither mode. Add distinct noise-pattern, quantization/color-space,
  nonuniform palette-position and input-analysis options with documented
  semantics. `scale` currently enlarges threshold cells; it is **not**
  downsampling or an analysis-resolution control. Color selection and spatial
  pattern should be independently configurable.
- **Temporal stability:** fixed Bayer and seeded blue-noise patterns are
  deterministic and spatially anchored by default. Any optional temporal
  variation must be explicitly selected, reproducible at arbitrary/out-of-order
  project timestamps and checked for objectionable crawling/sparkle. Never
  rely on previous-frame state, wall clock or random per-frame seeds.
- **Video-ready alpha/color:** account for straight/premultiplied alpha,
  transparent borders, source-color luminance, interpolation gamut handling,
  scaling/crops, nested groups/masks, arbitrary palette hue, and CPU/WGPU
  tolerances. Give unambiguous names/units and test invalid combinations.
- **Reference is a quality target:** AcerolaFX example4 inspires fine texture,
  clear silhouettes and controlled shadows across user-chosen palettes, not
  a hardcoded crimson look, an exact shader port or an automatic pixel-perfect
  comparison against different footage. Use repository-generated fixtures
  and inspect renders in addition to numerical assertions.
- **Validation gates:** for each meaningful new control, prove a distinct
  visual effect, CPU/WGPU consistency, reasonable 1080p cost and 4K resource
  behavior before marking it complete. Existing 1–4 checkboxes remain open
  until their documented verification is finished; extension work does not
  retroactively mark them done.

### 5. Advanced dithering and palette mapping

- [x] Establish baseline renders from repository-generated source footage and
  the existing `OrderedDither`/`PaletteMap` API. Capture gaps against the
  [fine-detail reference](../../development/stylization/dithered-palette-look.md)
  rather than assuming an exact reference shader configuration.
- [x] Add reproducible **blue-noise dithering** alongside Bayer, with authored
  selection, seed/tile/origin policy and optional validated custom threshold
  textures if resource contracts can be kept portable. Preserve original
  Bayer behavior; no accidental temporal re-randomization.
- [x] Support distinct **quantization modes**: current luminance-indexed
  palette, RGB/channel-count quantization, at least one hue-aware mode and
  perceptual nearest-palette matching. Specify color-space conversions,
  rounding, tie-breaking, gamut treatment and alpha policy consistently
  across CPU/WGPU.
  - [x] Luminance, RGB nearest-palette and integer hue-aware matching.
  - [x] Independent RGB channel-count quantization, 2–256 levels per channel.
  - [x] Perceptual nearest-palette matching.
- [x] Support **nonuniform tonal palette stops** and controllable palette
  interpolation in an appropriate color space (e.g. RGB and OKLab); preserve
  stops/order under animation and document chromatic vs luminance modes.
  - [x] Static nonuniform positions for tonal gradient/nearest mapping and dithering,
    preserved through phase animation; legacy uniform output retained.
  - [x] Controllable encoded RGB/Oklab interpolation space.
  Permit carefully bounded deterministic palette generation only if useful.
- [ ] Add **tone response and detail controls** that operate on the signal
  *entering quantization*: gamma/curves or shadow-mid-highlight shaping,
  local contrast and optional edge/detail preservation. Share existing
  `ColorAdjust` where sufficient; don't alter original source colors
  unintentionally to change threshold eligibility.
  - [x] Bindable entering-signal exposure/gamma using existing ColorAdjust, with
    the original retained for blending; focused tonal CPU/WGPU acceptance.
  - [ ] Local contrast and optional edge/detail preservation.
- [ ] Make filtered input/analysis resolution **independent** of threshold
  pattern size. Define nearest/linear/area filtering, lattice alignment,
  aspect ratio/partial borders and texture-budget limits. Keep true
  output-pixel fine dithering available at `scale=1`.
- [ ] Validate public Rust/JSON/Python descriptors, type hints, animation
  bindings, defaults, invalid inputs and legacy behavior; implement both CPU
  and WGPU with no readbacks or unbounded frame allocations.
- [ ] Compare Bayer/blue noise, tonal settings, quantization modes and
  resolution/detail trade-offs on the **same** grayscale/texture/silhouette
  fixtures in monochrome plus two chromatic palettes. Inspect contact sheets,
  moving previews, 1080p performance and 4K resource estimates.

Completion: nuanced, fine-detail and coarse creative dither styles are
reachable through documented controls, across arbitrary user palettes, with
provable visible quality and preserved legacy defaults.

### 6. Advanced customization of ASCII, halftone, sorting, CRT and looks

- [ ] **ASCII/PseudoASCII:** diagnose dark/unreadable source-color output using
  repository synthetic/video fixtures; distinguish expected low tone from
  defects. Add independent glyph-selection tonal response, coverage-calibrated
  density where justified, source-color brightness/luminance compensation,
  glyph scale/spacing and finer fill-versus-edge control. Preserve deliberate
  dark-space rendering and custom font/character compatibility.
- [ ] **Halftone:** add selectable dot geometry (at least circle, ellipse and
  line), independent angle/channel-screen controls and print-response/dot-gain
  options. Assess a CMYK-style mode only with explicitly defined conversions,
  resource accounting and testable printed appearance. Retain original RGB
  modes and analytic antialiasing.
- [ ] **PixelSort:** allow additional sort measures (such as luminance, hue,
  saturation and selected channel), explicit run-selection/segment behavior
  and useful bounded region/mask constraints. Preserve stable ties, segment
  bounds, thresholds and intentional discontinuities; reject unsupported
  state or unconstrained whole-frame sorting.
- [ ] **CRT:** add selectable phosphor masks (stripe, grille, shadow-mask),
  scanline profile/width and restrained color bleed/distortion controls. Keep
  noise periodic when requested and geometry/filtering alpha-safe; verify
  that advanced modes remain visibly distinct at 1080p.
- [ ] Extend **composable, editable looks** using existing effect-recipe
  infrastructure (e.g. cinematic dither, retro terminal, comic print, VHS
  monitor). Do not create a second presets system; users can still edit every
  underlying ordinary effect and use it on videos/global stacks.
- [ ] Define ergonomic defaults, explicit opt-in for expensive options,
  backward-compatible serialization and bindable-vs-discrete controls.
  Verify all options on CPU and WGPU, with visual source detail, transparency,
  deterministic temporal behavior, 1080p performance and 4K resource checks.

Completion: each effect family offers materially useful artistic controls and
combinations without sacrificing deterministic rendering or existing behavior.

### 7. Showcase, regression and documentation

- [ ] Add a per-mode **visual acceptance matrix** covering new patterns,
  quantizers, palette stops, calibrated glyph/edge controls, halftone shapes,
  sorting metrics, phosphor masks and composed looks. Compare real rendered
  frames (not merely successful jobs or CPU/WGPU parity) against source detail.
- [ ] Use repository-generated FFV1 showcase footage and deterministic
  grayscale, shadow/highlight, edge, alpha and moving-shape fixtures; do not
  depend on a user's private video. Reject unexpectedly uniform/black output
  on appropriately exposed source, even when both backends agree.
- [ ] Publish editable reference-inspired recipes and contact sheets with
  **three palettes**, Bayer vs blue noise, tonal mappings, detail settings
  and distinct analysis resolutions, with source-based side-by-side results.
- [ ] Add licensed/synthetic deterministic examples for advanced ASCII,
  custom font/characters, rainbow loops, both pixel-sort directions,
  halftone/CRT and composed preset/recipe looks.
- [ ] Demonstrate a repeating music-background scene; separately note
  footage/audio-loop requirements.
- [ ] Produce short deterministic image/frame-sequence temporal diagnostics
  across static controls, moving subjects, slow tonal threshold sweeps,
  procedural loops, effect transitions and intentionally discontinuous
  effects. Record visual/contact-sheet inspection, appropriate metrics,
  expected-vs-unwanted changes and actual CPU/WGPU adapter results.
- [ ] Run focused/full Rust/Python/schema/docs checks, software WGPU tests,
  hardware WGPU tests where available, and compare measured 1080p workloads
  and 4K/resource-limit behavior against baselines.
- [ ] Update [effect guide](../../guides/python/effects.md),
  [effect reference](../../reference/effects.md),
  [feature support](../../reference/feature-support.md), API examples and
  [asset ledger](../../../examples/showcase/ASSETS.md).
- [ ] Record test results and any blocked hardware-only checks; move plan
  to `completed/` only when all implementation acceptance is fulfilled.

Completion: reproducible visual showcase and honest CPU/WGPU capability claims.

## Feature acceptance checklist

- [ ] All agreed families and modes exist in Python/Rust/JSON as applicable,
  with defaults, bounds, documented scope and invalid-input tests.
- [ ] Built-in characters plus arbitrary user character strings/custom font
  resources validate, render and fail sensibly across both backends.
- [ ] CPU and actual WGPU frames compare with reasoned tolerances, including
  transparency, edge/cell boundaries, masks, groups and effect stacking.
- [ ] Positive finite explicit loop periods work for supported procedural
  animation; output at times separated by one period matches with nonsequential
  evaluation; source/audio looping is a separate concern.
- [ ] Animated palettes/rainbow/color modes and supported audio-reactivity
  use authored project-time signals/parameters deterministically.
- [ ] Both horizontal and vertical segmented pixel sorts have bounded
  resources, deterministic ties and CPU/WGPU coverage.
- [ ] 1080p representative performance and memory are measured; 4K rendering
  is tested where supported, with clear device-limit diagnostics otherwise.
- [ ] Optional looks reuse existing image-only presets where valid and
  composable effect chains for footage/global output.
- [ ] Dithered palette styling matches the visual detail and tonal structure
  of its design reference with at least three meaningfully different palettes;
  reject coarse/noisy low-detail outputs regardless of color choice.
- [ ] Advanced dither modes (Bayer/blue noise; luminance, channel and
  perceptual palette quantization), uneven palette stops, independent analysis
  resolution, and tone/detail controls have explicit semantics and distinct
  visually verified results on all supported renderers.
- [ ] ASCII tonal/density/color compensation, halftone dot/screen variants,
  bounded sorting metrics, CRT phosphor/scanline variants and reusable looks
  cover documented examples without changing legacy project defaults.
- [ ] Generated video and grayscale/alpha fixtures guard against apparently
  uniform-black output on well-exposed input; tests evaluate actual output
  structure, not solely successful encoding or CPU/WGPU numerical parity.
- [ ] Dither/ASCII/halftone/CRT patterns obey documented spatial anchoring
  and pixel/border rules under scaling/transforms, with area-based analysis
  and antialiased geometry where required; intentional crispness remains.
- [ ] Interpolable parameters and applicable procedural noise evolve
  continuously under project-time and loop-period tests; random-access
  frame evaluation is repeatable. Discrete configuration transitions are
  either intentionally stepped or given documented compatible blending.
- [ ] Pixel sorting maintains stable ties/segmentation with both directions;
  threshold-edge changes are deterministic and distinguishable from expected
  source-driven glitch motion.
- [ ] Reproducible automated temporal diagnostic sequences and human-reviewed
  render comparisons cover stable/animated footage and known discontinuities;
  unintended flicker is investigated with CPU/WGPU evidence, not declared
  absent solely from aggregate metrics.
- [ ] Licenses/attribution for code, fonts, glyphs and showcase footage are
  preserved, with reproducible example inputs.
- [ ] Targeted tests plus `just check`, `just python-test`,
  `just docs-check`, `just wgpu-software` and
  hardware verification where available are reported accurately.

## Progress and verification

The initial **product choices** and Milestone 1 technical contracts were
recorded on 2026-10-08. The following historical checkpoints describe staged
implementation evidence; later dated entries supersede earlier limitations.
As of the 2026-10-09 advanced-customization extension, all original families
have first implementations but Milestones 2–7 still need their respective
verification and expanded artistic-control acceptance. Do not infer completion
from the earlier 'initial palette/dither' checkpoint or passing CI alone.

### Current implementation session (2026-10-08)

- Clean starting revision `2b42f0e`; work remains exclusively on
  `feat/stylized-video-effects`. ASCII and the remaining stylization families
  are being implemented in parallel with narrow edits to shared registries.
- Adapter discovery (`just wgpu-list`): Vulkan llvmpipe is **software**;
  GL `D3D12 (NVIDIA GeForce GTX 1650 SUPER)` is classified **discrete GPU**.
  Use `VESTRA_WGPU_BACKEND=gl`; Vestra does not read `WGPU_BACKEND`.
- Actual GL palette/dither test run: seven passed, one composed-mask test
  initially failed, one explicit resolution test ignored. The failure exists
  in the **unstyled control**, where three near-transparent pixels lose alpha
  at device UNORM conversion. Explicit byte rounding in the mask shader is
  under regression verification; retain control-based composed tolerances.
- Explicit ignored `gpu_stylization_1080p_and_4k` test executed on GL:
  **passed**, exact CPU/WGPU frames at both 1920×1080 and 3840×2160.
- Visual skill comparison of the 640×360 ember pair: raw RGBA MAE **0**, max
  error **0**, mismatch fraction **0**. Inspected source/monochrome/ember/ocean
  contact sheet under `target/stylization/contact.png`: readable silhouette,
  fine line contours, calm dark regions, structured midtone dithering; palette
  changes retain identical index geometry. This synthetic evidence does not
  substitute for the remaining moving-footage and complete suite acceptance.
- Temporal tests/tooling are being added for stationary controls, moving
  transforms, monotonic threshold sweeps, procedural seams and random access.
- Performance records are in `target/benchmark-results/stylization-initial/`;
  final source-stable suites will be measured after implementation settles.
- `uv run --no-project python scripts/check-docs.py`: **passed**, 292 links.

### Restricted-environment continuation (2026-10-09)

- The session changed to a read-only `.git` and restricted network/device
  environment. Commits/pushes and CI on the new working tree cannot be made
  under those permissions; no approval escalation or workaround is attempted.
- Hardware GL adapter creation now fails with EGL `failed to create dri2
  screen` and `no compatible adapter`, including a previously passing literal
  ramp test binary. Earlier GL results above are valid only for the initial
  palette/dither code; **new-family hardware verification is blocked**.
- Vulkan llvmpipe remains available. A strict rendered literal ramp test
  executed successfully on that **software** adapter. New effects will use
  this path for actual software WGPU parity until hardware access is restored.
- `cargo test -p vestra-core --lib`: **314 passed**, no failures.
- Benchmark/diagnostic tooling tests: **22 passed**. Added complete-effect
  smoke/1080p suites and source fingerprinting of canonical effect fixtures.
- Added asset-free canonical examples for all new families and an original
  periodic FFV1/sine showcase. Its FFmpeg source-generation step ran at 64×48
  / 8 fps; public authoring and render acceptance are still pending integration.

Milestone checkboxes remain open until all relevant implementation, visual,
public-contract and performance acceptance has run. Comprehensive checks,
CI and native Windows backend-specific validation are **unverified** so far.

### Remaining stylization implementation evidence (2026-10-09)

- Added Halftone, PixelSort and Crt end to end across canonical descriptors,
  validation, compilation/evaluation, Rust CPU/WGPU passes and both public
  Python authoring layers. Their bindable controls reuse existing properties,
  signals and keyframes. Enum/color/integer changes remain discrete.
- Halftone uses exact alpha-weighted pixel-area statistics in rotated cells,
  including partial borders; one metadata temporary and one resolve pass.
  RGB screens analyze independently rotated channel lattices. Metadata uses
  shared integer half-up quantization, avoiding CPU/WGPU float rounding ties.
  CPU sums are O(pixels), with sparse populated-cell statistics and cached
  representatives; GPU cell sums require no readback or extra full-frame blur.
- PixelSort has stable ascending/descending ties, horizontal/vertical runs,
  inclusive thresholds and bounded partial blocks. Its 64-lane GPU bitonic
  sort uses 7 KiB shared storage and next-power-of-two block work; input is
  intentionally not blurred. Dispatch and device/storage limits are checked.
- Crt uses alpha-aware inverse sampling, antialiased borders, analytically
  integrated scanlines, output-pixel phosphor masks and seeded continuous
  owner-time sine/cosine motion. Explicit period and modulo-normalized phase
  repeat independently of source/audio. Pure black/white halftone endpoints
  and default alternating CRT scanlines have focused quality regressions.
- Added ordinary effect recipes `halftone_print`, `analog_monitor` and
  `sorted_neon`, plus reference/guide updates. No separate preset engine.
- Focused evidence: CPU analog **6 tests passed**; huge authored CRT phase
  periodicity and invalid canonical-contract regressions passed; public Python
  analog **8 tests passed**;
  effect typing and full-package Ruff passed; core/render all-target Clippy
  with warnings denied passed.
- Actual Vulkan llvmpipe **software** WGPU tests passed: all-family intensity
  sweep/random-access frames, CRT seed/period seams and both sort directions/
  orders with literal ties and threshold membership. Refreshed visual skill
  diagnostics and inspected halftone/CRT contact sheets. Halftone intensity
  sequences had max channel error **1**, worst RGBA MAE **0.09327**; six CRT
  seam frames matched exactly. Full temporal coverage, 1080p/4K measurements
  and new-family hardware verification remain separate acceptance work.

## Decisions and discoveries

- 2026-10-09: Extend this feature branch with artistic customization across
  all families, centered on reproducing fine-reference dithering through
  blue noise, quantization/color-space modes, nonuniform palette stops,
  independent analysis resolution and tone/detail preservation. Add advanced
  ASCII, halftone, PixelSort and CRT controls plus ordinary editable looks.
  Gate acceptance on actual source-specific visual quality and deterministic
  repository-generated video tests, not parity or encoding success alone.
  Preserve backwards-compatible defaults and both renderers.
- 2026-10-08: Add effect-specific temporal-stability and visual-quality
  requirements throughout Milestones 2–5: pattern anchoring, appropriate
  prefiltering and area averages, antialiasing, selective threshold stability,
  smooth parameter/noise motion, stable segmentation, multi-resolution
  analysis, regression diagnostics and deliberate configuration transitions.
  Prefer stateless deterministic approaches and reuse current render passes;
  do not blur intentional pixel-art/glitch aesthetics.
- 2026-10-08: AcerolaFX example4 is a **fine-detail dither/palette quality
  reference**, not a fixed red palette or ASCII requirement; changing the
  authored palette must preserve the effect's pattern and tonal structure.
- 2026-10-08: Reuse the canonical effect descriptors, core pass planner and
  bounded render resources; extend only for justified needs.
- 2026-10-08: Keep effects reusable beyond music mixes.
- 2026-10-08: User approved every planned effect family, advanced ASCII
  customization, all color modes/animation, both renderers and clip/global
  placement, and 1080p-first/4K-capable rendering.
- 2026-10-08: User approved bounded horizontal **and vertical** pixel sorting,
  optional looks, existing preset reuse plus effect composition, custom fonts
  and characters, licensed open-source references, and explicit loop periods.
- 2026-10-08: Existing cinematic preset implementation is image-only and
  one-per-image; generic stylization must not silently inherit that limit.

## Completion / handoff

Not complete. Initial families and their public interfaces are implemented;
Milestones 5–6 add approved advanced controls and Milestone 7 integrates
visual/temporal/performance acceptance. Finish feature-specific verification,
hardware quality/resource checks, comprehensive checks and serial 1080p
measurements before closing acceptance and moving this plan.

### Restored environment and public examples (2026-10-09)

- Filesystem/network restrictions were removed. Git writes are available;
  GL adapter discovery again identifies D3D12 on NVIDIA GeForce GTX 1650 SUPER
  as hardware. The preceding restricted-environment blockers are historical.
- Twelve public showcase variants now pass random-access and four-second
  repeat checks, including custom-font ASCII and three composed recipes.
  Custom-font preparation exposed a font reference in the wrong asset collector;
  corrected it in `collect_fonts` and added a compiler regression.
- Rendered the complete custom-font ASCII smoke scene on hardware WGPU:
  320×180, 48 H.264 frames over eight seconds, AAC at 48 kHz. Inputs are original
  periodic FFV1 footage and a generated periodic soundtrack.
- Device requirements tests: **22 passed**, including atlas dimensions/mips,
  sampled texture/binding limits, and 64-lane/7 KiB sorting workgroups.
- Documentation link check: **307 links, zero missing**. Broader checks found
  a stale public export expectation and an existing SDK test with an empty
  visual timeline; updated the export contract and supplied a four-second
  source clip in the test. Focused public tests **23 passed**, SDK test passed.

### PR checkpoint (2026-10-09)

- CI at `eff57dd` passed Style, Rust, Python and CI Success:
  https://github.com/evgen2571/vestra/actions/runs/37892264547.
- Fixed composed mask/matte restoration validation for `EffectB`; all 28
  frame-plan tests and all eight composed hardware effect families passed.
- Custom-font registration and validation now cover transition presentations
  and owned mask groups. Focused Python tests: 68 passed; Rust transition
  tests: 16 passed, plus all-scope font regression.
- All eight families passed actual 1080p comparison; seven passed 4K before
  CRT exposed transparent-border arithmetic differences. The CRT fix uses
  bounded shared Q15 inverse coordinates and prepared jitter coefficients,
  a 1/256-pixel sampling lattice, explicit half-up byte rounding and zero RGB
  when final alpha is zero. The focused hardware CRT test now passes at both
  1080p and 4K with the original raw RGBA tolerance (confirmed final run).
  Unit sampling regression, eight CPU analog tests and renderer Clippy passed.
- CRT 4K resource estimates: persistent 298601472 bytes, working textures
  165888000 bytes, staging 398134272 bytes. Estimates exclude driver overhead.
- PR opened as draft at the user's request while final canonical checks and
  source-stable serial CPU/hardware 1080p benchmark captures remain pending.
  No final acceptance or performance claim is made at this checkpoint.

### Advanced dither pattern implementation (2026-10-09)

- Milestone 5 starts from the unchanged Bayer8/encoded-luminance implementation.
  Actual 640×360 source/Bayer/blue-noise frames in monochrome, ember and ocean
  are captured under `target/stylization/frames/gl/`. Inspected
  `target/stylization/three-palette-patterns.png`: both patterns retain silhouettes,
  thin lines, gradients, dark regions and highlights. Blue noise removes the
  regular Bayer grid; shadow-subject contrast remains limited by the existing
  tone response. Independent tone/detail/analysis controls remain required.
- Added opt-in `matrix="blue_noise"` and discrete unsigned 32-bit `seed=0`
  across canonical Rust/JSON, descriptors/schema, advanced/high-level Python,
  compile/evaluate/pass IR, CPU and WGSL. Omitted seeds preserve old projects;
  Bayer ignores seeds and retains its existing defaults and literal output.
- Pattern contract: original 32×32 toroidal void-and-cluster rank tile, 1024
  thresholds `(rank+.5)/1024`, anchored to output pixels after integer `scale`.
  Fold seed with `seed ^ (seed >> 13) ^ (seed >> 26)`; use low 10 bits for tile
  origin, then transpose/x/y-reflection bits. Seeds select transforms and can
  alias. No per-frame randomization, temporal state, extra texture/buffer binding,
  readback, pass or frame allocation. Custom threshold textures are not added:
  the optional path would need a new portable asset/preflight contract.
- `scripts/generate-blue-noise.py` reproduces both renderer rank tables and
  asserts permutation/coverage and suppressed low-frequency energy at 12.5%,
  25%, 50%, 75% and 87.5% cutoffs. Tables are project-original MIT work.
- Numeric descriptor defaults now retain canonical JSON types through
  `EffectParameterDefault::{String,Integer}` (also exported by the Rust SDK).
  This prevents generic authoring from emitting string `"0"` for integer seed.
  Existing enum defaults retain their serialized values.
- Tests first failed on the missing seed constructor and `blue_noise` variant.
  Focused Python palette/showcase tests: **49 passed**. CPU tone-coverage/alpha
  and legacy Bayer-seed checks: **2 passed**. Seeded arbitrary-time actual-frame
  parity: **passed with max RGBA error 0** on GL D3D12/NVIDIA GTX 1650 SUPER
  (**DiscreteGpu**) and Vulkan llvmpipe (**Software**). Three palettes retain
  exactly identical tone-index geometry on hardware.
- Explicit hardware 1080p/4K test ran both Bayer8 and blue noise: **passed**
  with exact CPU/WGPU output and unchanged opaque support. Resource estimates
  are saved with each fixture; blue noise adds no frame-sized working resource.
- Editable examples now accept Bayer/blue-noise and seeds; the generated FFV1
  showcase adds `dither-blue-noise` and `--palette mono|ember|ocean`. Rendered
  and inspected 320×180 ember CPU and ocean hardware-WGPU video frames; videos
  contain 48 H.264 frames over eight seconds with the generated AAC soundtrack.
- Renderer/core/CLI all-target Clippy and focused Ruff passed. Schema freshness
  and schema validation passed; docs: **308 links, zero missing** at this point.
  A full check found a stale matrix-enum expectation; updated it with blue-noise
  and integer-seed bounds/default checks. Full verification and source-stable
  1080p pattern benchmarks are running, not yet claimed passed.
- Milestones 5–7 and all broader acceptance items remain open. Next: finish
  pattern verification/performance capture, then define and implement the
  quantization/color-space, uneven stops, tone/detail and independent analysis
  contracts before advancing to the other-family controls.

- Full contributor checkpoint: `VESTRA_WGPU_BACKEND=gl just check` **passed**
  (1043 Rust tests passed, 16 ignored; explicit pattern 1080p/4K check was run
  separately). `just python-test` **760 passed**, compileall passed. Added blue-noise
  typing usage and ran the four focused effect typing checks successfully.
- The first strict hardware benchmark capture reproduced two recorder bugs:
  SDK render-result JSON omits the selected adapter, and the old benchmark
  classification used raw `device_type=other` rather than Vestra's existing
  GL/D3D12 hardware classification. Fixed benchmark-only sample serialization
  and reused `AdapterMetadata::performance_class()` for both logging and runner
  validation. No renderer/SDK serialization or hardware policy was broadened.
  New negative tests still reject software even with a hardware raw device type;
  benchmark-tool tests **26 passed**, bench Clippy/build passed. Hardware/CPU
  captures are being regenerated with that recorder and frozen sources.

- Source-stable serial `dither-patterns-1080p` captures now completed with the
  corrected benchmark recorder; both report source fingerprint `53b10617c7d5…`.
  Same original FFV1 input, 1920×1080/30 fps, 90 frames, one warmup and three
  samples per workload. CPU/hardware captures are respectively under
  `target/benchmark-results/dither-patterns-cpu-recorded-20261009/` and
  `target/benchmark-results/dither-patterns-hardware-recorded-20261009/`.

  | Workload | CPU median wall ms (range) | Hardware GL median wall ms (range) |
  | --- | --- | --- |
  | No-effect FFV1 control | 3375 (3368–3424) | 2604 (2594–2631) |
  | Bayer8 | 4170 (4124–4197) | 2463 (2450–2526) |
  | Blue noise, seed 37 | 4263 (4201–4265) | 2447 (2424–2463) |

  All hardware samples selected GL D3D12/NVIDIA GTX 1650 SUPER, classified
  `discrete_gpu`, with no CPU fallback. These are end-to-end preview/H.264
  measurements, including preparation, decoding and encoding; they do not
  isolate shader time or promise real-time performance. GPU aggregate frame
  work medians: control 792 ms, Bayer 904 ms, blue noise 940 ms per 90 frames.
  CPU aggregate frame work is concurrent across eight workers and exceeds
  wall time; it is not per-frame latency. Blue-noise CPU wall cost is about
  2.2% above Bayer in this capture; hardware wall sample ranges overlap.
- Pattern resource estimates match Bayer exactly: 1080p persistent/staging
  82,947,840 / 107,831,040 bytes; 4K 331,779,840 / 431,312,640 bytes for the
  composed palette-plus-dither fixture. Driver/program overhead is excluded.
- Latest palette/benchmark regression tests: **63 passed**. Dedicated full
  `just wgpu-software` / `just wgpu-hardware` recipes remain pending for final
  branch acceptance; focused software/hardware rendering and the complete GL
  contributor checkpoint above are separate evidence.

### Chromatic palette matching increment — 2026-10-09

- Added opt-in `nearest_rgb` and `nearest_hue` to Palette Map and Ordered Dither,
  through Rust/canonical JSON/descriptors/Python/CPU/WGPU. Existing luminance,
  gradient and rainbow contracts remain unchanged. RGB uses encoded byte-space
  squared distance. Hue-aware matching uses integer HSV, circular hue weighted
  by shared saturation, and squared saturation/value differences. Full formulas,
  rounding, ordered ties, gamut and alpha rules are in the effects reference.
- Chromatic dithering chooses the two nearest entries using inverse squared
  distance probabilities, rounded to 1/4096 and scaled by strength. Zero strength
  is ordinary nearest matching; an exact palette input never dithers away from
  that entry. Duplicate entries and equal-distance ties have defined behavior.
  These modes do not yet provide perceptual matching or channel-count controls;
  the Milestone 5 quantization checkbox remains open.
- Palette features are prepared once per effect. WGPU adds 64 bytes of packed
  features inside the existing 256-byte parameter record, with no new bindings,
  passes, frame textures or readbacks. Legacy pattern resource contracts remain.
- Focused public tests: **42 passed**, including exact palette preservation for
  both new modes/effects and a literal dark-red example distinguishing RGB from
  HSV matching. Core/CPU regressions verify hue wrapping, achromatic behavior,
  ordered ties, duplicate entries, zero/full strength and transparent RGB/alpha.
- Actual frame tests passed with **zero byte error** on hardware GL/D3D12 NVIDIA
  and software Vulkan llvmpipe: legacy/RGB/hue map, Bayer8 and blue noise on the
  same detailed scene, plus literal distance cases and repeated timestamps.
  Explicit new-mode 1080p/4K hardware checks also passed with exact CPU parity.
  For one dither effect, persistent/staging estimates are 74,652,672 / 99,535,872
  bytes at 1080p and 298,601,472 / 398,134,272 at 4K (driver overhead excluded).
- Inspected `target/stylization/chromatic-contact.png`,
  `chromatic-4k-contact.png` and `chromatic-ember-video-contact.png`. RGB and hue
  modes preserve visible silhouettes, highlights and fine lines. Hue-aware
  matching can move midtone brightness substantially, especially with a saturated
  palette; it is an artistic HSV metric, not a perceptual-lightness guarantee.
  Two generated ember previews have 48 H.264 frames each and animated palette
  phase over the periodic moving source. Full motion/mask/loop-seam acceptance
  for every new mode remains pending.
- Editable examples accept `--mode nearest|nearest_rgb|nearest_hue`. Added
  dedicated `chromatic-dither-smoke`/`chromatic-dither-1080p` suites comparing
  control/luminance/RGB/hue over identical FFV1 inputs without changing old suites.
  Benchmark-tool tests **26 passed**, focused Ruff passed, docs links **308/0**.
  Full contributor/Python checks and source-stable 1080p measurements are still
  being completed; their outcomes are not implied by these focused checks.

- Chromatic contributor checkpoint completed: `VESTRA_WGPU_BACKEND=gl just
  check` **passed** (1046 Rust tests passed, 17 ignored), including workspace
  all-target/all-feature Clippy, schema validation/freshness and docs checks.
  `just python-test` **765 passed**, compileall passed. Ignored full-resolution
  tests are distinct from the explicit new-mode 1080p/4K run recorded above.
- Source-stable serial chromatic benchmarks completed with one release binary:
  source `c4ee7e62c027…`, executable `54affc699424…`. Reports are under
  `target/benchmark-results/chromatic-dither-{cpu,hardware}-20261009/`. Same
  original FFV1 input, 1920×1080/30 fps, 90 frames, one warmup and three samples.

  | Workload | CPU median wall ms (range) | Hardware GL median wall ms (range) |
  | --- | --- | --- |
  | No-effect FFV1 control | 3441 (3380–3444) | 2532 (2389–2534) |
  | Luminance blue noise | 4243 (4188–4260) | 2448 (2444–2519) |
  | RGB blue noise | 4913 (4846–5003) | 2489 (2434–2495) |
  | Hue-aware blue noise | 5133 (5093–5237) | 2506 (2430–2536) |

  All hardware samples selected GL D3D12/NVIDIA GTX 1650 SUPER, classified
  `discrete_gpu`, without fallback. RGB/hue matching cost approximately 16%/21%
  more CPU wall time than luminance in this capture. Hardware ranges overlap;
  aggregate GPU frame-work medians are 860/950/959/941 ms respectively per
  90 frames. These include preparation, decoding and H.264 encoding, are not
  isolated shader measurements, and make no general real-time guarantee.
- Remaining work: channel-count/perceptual matching, uneven stops/interpolation,
  tone/detail and independent analysis controls; then Milestone 6 extensions and
  the full Milestone 7 per-mode motion/mask/loop/performance acceptance. Dedicated
  full `just wgpu-software`/`just wgpu-hardware` recipes remain pending. No broader
  milestone checkbox was closed on the strength of this increment alone.

### RGB channel-count increment — 2026-10-09

- Added `rgb_channels` / `PaletteMode.RGB_CHANNELS` to Palette Map and Ordered
  Dither, with static `levels=4`, bounded to integer 2–256. Channels are quantized
  independently in encoded byte space to a uniform RGB cube; the same stationary
  rank drives each channel. Palette/phase/period are ignored in this mode but
  retain normal validation. Other modes ignore levels and keep legacy behavior.
  Nearest rounding, Q12 dither probabilities, final byte rounding and alpha
  policy are specified in the effects reference. At 256 levels, all strengths
  preserve source bytes exactly.
- Canonical Rust/JSON, descriptors, validation, compiled/evaluated passes, CPU,
  WGSL and both Python authoring APIs propagate levels. The advanced-builder
  regression caught missing convenience-method arguments; updated both methods
  and verified atomic editing/rejection. The integer is stored in an existing
  padding word; parameter record remains 176 bytes inside the 256-byte allocation.
  There are no added passes, bindings, frame textures, readbacks or allocations.
- Focused verification passed: **322 core tests**, **89 Python palette/API/type
  tests**, workspace all-target/all-feature Clippy, schema validation/freshness,
  Ruff and **308 docs links / zero missing**. CPU coverage checks validate the
  blue-noise channel means, zero/full strength, 256-level identity, alpha and
  hidden transparent RGB. Invalid levels and defaults are tested for both effects.
- Actual GL/D3D12 NVIDIA and software Vulkan llvmpipe rendered-frame comparisons
  pass with **zero byte error** for 2/4/8/256 levels with mapping, Bayer8 and blue
  noise. Repeated timestamps are identical; 256 levels equal the original source.
  Explicit 1080p/4K hardware checks pass at both 2 and 256 levels. Single-effect
  resource estimates remain 74,652,672 / 99,535,872 persistent/staging bytes at
  1080p and 298,601,472 / 398,134,272 at 4K, excluding driver overhead.
- Inspected `target/stylization/channel-contact.png` (source vs 2/4/8 levels) and
  `channel-video-frame.png`. All levels retain recognizable shapes, highlights,
  fine lines and source hue, with visibly different coarseness. Editable examples
  accept `--mode rgb_channels --levels N`. The generated moving-preview artifact
  `target/stylized-showcase/smoke/dither-blue-noise-rgb_channels-4-cpu.mp4` contains
  48 H.264 frames at 320×180 over eight seconds. Full motion/loop/mask/stacking
  acceptance is still open rather than inferred from these sample frames.
- Source-stable serial `channel-dither-1080p` measurements used the same release
  executable and 90-frame 1920×1080/30 fps FFV1 input, with one warmup and three
  measured samples. Source `4990a890b82f…`, executable `8751da62e324…`; reports
  under `target/benchmark-results/channel-dither-{cpu,hardware}-20261009/`.

  | Workload | CPU median wall ms (range) | Hardware GL median wall ms (range) |
  | --- | --- | --- |
  | Luminance blue noise | 4245 (4222–4271) | 2530 (2491–2544) |
  | Four-level RGB channels | 4726 (4697–4742) | 2483 (2472–2522) |

  CPU wall cost is about 11.3% higher; hardware ranges overlap and aggregate GPU
  frame work is 938/940 ms per 90 frames. Every hardware sample selected the
  GL D3D12/NVIDIA GTX 1650 SUPER adapter, classified `discrete_gpu`, without
  fallback. These end-to-end measurements include decoding/preparation/H.264
  encoding and do not isolate shader cost or promise real-time performance.
- Perceptual quantization remains the next missing quantizer. The original
  [Oklab definition and linear-sRGB conversion](https://bottosson.github.io/posts/oklab/)
  was inspected as its primary mathematical source. A bounded integer conversion
  is being considered to make discrete color selection reproducible across CPU
  and WGSL; approximation precision and tie behavior must be verified before
  exposing it. No perceptual mode is implemented or claimed yet. Uneven stops,
  tone/detail/analysis controls and Milestones 6–7 remain open. The full branch
  contributor/Python and dedicated GPU acceptance recipes must be rerun after
  the remaining changes; the prior full checkpoint predates channel controls.

### Perceptual quantization increment — 2026-10-09

- Added `nearest_oklab` / `PaletteMode.NEAREST_OKLAB` to both palette effects and
  all canonical Rust/JSON/descriptors/Python/CPU/WGPU paths. It uses squared
  Euclidean distance in an explicitly documented fixed-point Oklab approximation
  of sRGB/D65 input. The original Oklab and CSS Color 4 definitions are linked
  in the effects reference, generator and third-party notices.
- A 256-entry sRGB Q16 table and normalized Q15 matrices are generated once with
  60-decimal arithmetic by `scripts/generate-oklab.py`, for Rust and WGSL from
  the same numeric source. Cube roots use bounded integer binary search and
  Q10 rounding; Lab uses 1/1024 coordinates with ties away from zero. Packing
  is widened to 11/10/10 bits inside the same 64-byte feature array and existing
  176-byte parameter record. No frame textures, bindings, passes or readbacks
  are added. Source alpha, palette order and legacy modes remain unchanged.
- Exact RGB palette matches take priority over rounded feature ties. A cyan
  pair differing by one red byte shares Q10 features; dedicated CPU and actual
  GPU partial-alpha tests prove that each exact authored color survives. Other
  ties choose the first entry. Dither uses existing two-nearest Q12 probabilities;
  CPU uses a 64-bit numerator and WGSL bounded long division to prevent overflow.
- Independent float-reference tests cover a 16³ full-range grid, a 16³ shadow
  cube, every grayscale byte and all 65,537 Q16 cube-root inputs. Maximum sampled
  component error was **0.0034671877**, under the 0.004 regression limit; this is
  a sampled result, not an exhaustive global error bound. Every gray stays
  neutral and lightness monotonic. A separate release test checked packing bounds
  for **all 16,777,216 RGB triples**: component minima `[0,272,193]`, maxima
  `[1024,796,715]`; all fit the fixed record, no gamut-feature clipping required.
- Shader parsing initially rejected the reserved WGSL name `target`; renamed
  the local and reran parser/runtime checks. The first full Python run loaded an
  extension built before that fix and reproduced the stale shader error in normal
  WGPU mask/text/runtime paths. Stopped that run, rebuilt the extension from the
  corrected sources, and verified the previously failing mask case (**passed**).
  That interrupted run is not a passing checkpoint; the rebuilt full suite is
  being verified separately.
- Focused public palette tests: **62 passed**, including exact authored colors
  and a literal gray-104 example choosing white in Oklab versus black in encoded
  RGB. Actual hardware GL/D3D12 NVIDIA and software Vulkan llvmpipe frame tests
  pass with **zero byte error** for map/Bayer8/blue-noise metric comparisons,
  literal lightness and rounded-feature ties with partial alpha, and repeated
  timestamps. Explicit 1080p/4K comparisons now include RGB, hue and Oklab and
  pass exactly. Resource estimates remain those of one ordinary dither pass.
- Hardware visual checks use monochrome plus ember/ocean palettes on the same
  scene, comparing mapping, Bayer8 and blue noise. Figure/background brightness
  contrast and surviving moon highlights are asserted, alongside exact parity.
  Inspected `target/stylization/oklab-contact.png`, `oklab-three-palette-contact.png`
  and `oklab-video-frame.png`. The saturated primary palette reveals a meaningful
  difference: Oklab retains the figure where encoded RGB mapping selects black.
  A coarse five-stop monochrome nearest map merged the figure/background into
  one stop; this is expected quantization, not a conversion defect. The mapping
  acceptance fixture uses nine monochrome stops. Palette granularity, dither and
  upcoming tone/detail controls remain necessary artistic choices.
- Editable examples expose `--mode nearest_oklab`; the generated eight-second
  ember moving preview uses ordinary editable palette phase/amount animation.
  Added separate perceptual-dither smoke/1080p workloads comparing luminance,
  encoded RGB and Oklab with identical source/palette/seed/sample settings.
  Workspace Clippy, focused Ruff and docs checks passed. Complete contributor
  and rebuilt Python checkpoints are running; source-stable 1080p performance
  capture follows after those competing workloads finish. The quantization
  checkbox remains open until these remaining gates are recorded.

- Perceptual contributor checkpoint finished: `VESTRA_WGPU_BACKEND=gl just check`
  **passed** (1053 Rust tests passed, 19 ignored). The exhaustive conversion and
  explicit 1080p/4K tests recorded above were separate runs. After rebuilding the
  native extension, `just python-test` **785 passed**, compileall passed. No stale
  extension failures were dismissed or counted as passed. The quantization
  implementation item is now closed; broader per-mode motion/stack/mask acceptance
  and the remaining Milestone 5 controls are still open.
- Source-stable serial `perceptual-dither-1080p` captures completed with source
  `e5b76dc30907…` and release executable `bdf66b9e2cfe…`. Same generated FFV1
  input, 1920×1080/30 fps, 90 frames, one warmup and three measured samples;
  reports under `target/benchmark-results/perceptual-dither-{cpu,hardware}-20261009/`.

  | Workload | CPU median wall ms (range) | Hardware GL median wall ms (range) |
  | --- | --- | --- |
  | Luminance blue noise | 4184 (4082–4215) | 2720 (2681–2798) |
  | RGB blue noise | 4788 (4735–4847) | 2718 (2681–2743) |
  | Oklab blue noise | 6031 (5950–6108) | 2742 (2740–2795) |

  Oklab's opt-in CPU wall cost is about 26% above RGB and 44% above luminance in
  this capture. Hardware ranges overlap; aggregate GPU frame work medians are
  859/873/879 ms per 90 frames respectively. Every hardware sample selected
  GL D3D12/NVIDIA GTX 1650 SUPER, classified `discrete_gpu`, without fallback.
  End-to-end timings include preparation, decoding and H.264 encoding and do not
  isolate shader cost or guarantee real-time performance. The legacy pattern
  mode remains the default; no expensive perceptual conversion is forced on CPU
  pixels in other modes.
- Next: nonuniform tonal stops and interpolation spaces, then tone/detail and
  independent analysis controls, before the Milestone 6 family extensions.
  Dedicated full software/hardware GPU recipes and complete Milestone 7 acceptance
  remain pending. No overall milestone or goal completion is claimed.

### Nonuniform tonal-stop increment — 2026-10-09

- Added optional static `stops` to PaletteMap/OrderedDither across canonical
  JSON/schema/descriptors, compilation/evaluation/pass operations and both Python
  APIs. Positions retain authored order and stay fixed while phase animates colors.
  `None` keeps the original uniform arithmetic. Custom positions apply only to
  gradient/nearest tonal modes; rainbow/chromatic/channel combinations fail with
  `VESTRA-PALETTE-STOPS` rather than silently ignoring the control.
- Positions round half up to the existing encoded-luminance grid `0..65280`.
  Validation requires matching cardinality, endpoints 0/1 and strictly increasing
  quantized positions, rejecting collapsed intervals. Gradient uses interval-local
  RGB interpolation; nearest chooses the upper stop at an exact midpoint. Dither
  uses interval-local Q12 fractions blended with the nearest choice by Q12 strength,
  selecting the upper stop when the rank threshold is at least `1-probability`.
  Source alpha/transparent RGB retain their existing policy.
- Kept the shared EvaluatedPalette (also used by ASCII) unchanged. Compiled/evaluated
  palette effects carry an optional 16-entry u16 position array. Packing expands
  the GPU parameter data from 176 to 240 bytes inside the existing 256-byte record:
  no additional pass, texture, allocation per pixel or readback. An initial shared
  array layout enlarged unrelated ASCII operations and failed the enum-size lint;
  moving positions onto the two relevant operations fixed that without disabling
  any checks or boxing render operations.
- Literal fixtures check black/red/white endpoints, gray-32 midpoint interpolation,
  nearest midpoint ties, half-tile Bayer/blue-noise coverage, strength zero and alpha.
  A first GPU assertion incorrectly expected every blue-noise column to contain
  half of each color. The corrected test measures the entire rank tile (512/1024
  red pixels); exact CPU/WGPU ramp parity is still required.
- Actual NVIDIA hardware GL and llvmpipe software Vulkan each passed both focused
  tests with byte-exact RGBA parity. Three nonuniform mono/ember/ocean scenes preserve
  figure contrast and repeat exactly at out-of-order times 0, .777s, 2s and 0.
  Explicit hardware 1080p/4K test passed both sizes with exact RGBA parity. Resource
  estimates remain 74,652,672/99,535,872 persistent/staging bytes at 1080p and
  298,601,472/398,134,272 at 4K, excluding driver metadata/padding.
- Inspected `target/stylization/nonuniform-three-palette-contact.png` and
  `target/stylization/nonuniform-video-frame.png`. Public generated-media example:
  `target/stylized-showcase/smoke/dither-blue-noise-ember-nonuniform-cpu.mp4`
  (320×180, 6fps, 8s), reproduced with `--stops 0 0.18 0.55 1`.
- Verification so far: core 323 passed/1 ignored; palette, high-level, advanced
  authoring and typing Python checks 174 passed; workspace/all-target/all-feature
  Clippy, schema checks and `just style` passed. A broad GL test invocation with
  default parallel test threads crashed with SIGSEGV. The canonical Rust test
  script already sets `RUST_TEST_THREADS=1`. The serial rerun logged passing
  cases through the final uniform-threshold test, but has no final result and
  is **unverified** as a complete run. No test process remains at handoff.
- Remaining: interpolation spaces, tone/detail and independent analysis-resolution
  controls, then Milestones 6–7. The combined stops/interpolation checkbox remains
  open; no whole-milestone or full-suite acceptance is claimed for this increment.

- Work paused at user request to commit the current worktree. The new
  tonal-stop benchmark scenarios build in release mode, but their timing runs
  have not been executed. Full final verification and remaining milestone work
  are still outstanding.

### Resumed measurement acceptance — 2026-10-09

- Resumed after commit `779f149`. Completed the previously pending
  `tonal-stops-1080p` suite on CPU and confirmed NVIDIA hardware GL, sequentially
  with no other render/test workload running during capture. Each scenario has
  one warmup and three samples, 90 frames at 1920×1080/30fps with FFV1 output.
- End-to-end wall medians/ranges (milliseconds): CPU uniform blue noise
  **5213 (4858–5229)**, nonuniform **4866 (4698–5048)**; hardware GL uniform
  **3271 (3053–3373)**, nonuniform **3154 (3100–3320)**. Ranges overlap, so these
  measurements do not establish a speed difference. Changed output colors can
  affect encoding cost; this is not an isolated shader-pass benchmark.
- Reports are `target/benchmark-results/tonal-stops-{cpu,hardware}-20261009/suite.json`.
  Both captured clean revision `779f149fd87edab50b22d598f3a60332be077727`, source
  SHA256 `45c8f1b33e112a5bf693d03d25cb9a05396ec4e2137a9f4cac39e5ff02b6995e`.
  Release executable SHA256: CPU
  `50890b27aec3a5e43bd40876954f888c78121b6fdbe65e91464c031085529336`, hardware
  `f1883cb813fb3c50e7a4e744ed14d8d4e9bcf4a87a387ea07cbdae2087948b73`.
  All hardware samples selected `wgpu`, GL, DiscreteGpu, adapter
  `D3D12 (NVIDIA GeForce GTX 1650 SUPER)`; no software performance claim.
- Next implementation remains interpolation spaces, followed by signal tone/detail
  controls and independent analysis resolution. Milestones 5–7 remain incomplete.
- Re-ran the previously unfinished final uniform-threshold diagnostic in isolation
  on hardware GL with `--test-threads=1 --nocapture`: **1 passed**, all six stages
  have zero RGBA channel differences, including fractional mapping/dithering and
  their chain. This proves that diagnostic independently; the earlier interrupted
  broad run still lacks a complete suite result and remains unverified as a run.


### RGB/Oklab palette interpolation increment — 2026-10-09

- Added `PaletteInterpolation::{Rgb,Oklab}`, canonical `interpolation` strings
  `rgb`/`oklab`, matching Python enums/properties and advanced convenience methods.
  The static control applies to PaletteMap gradient segments and authored palette
  color motion for both effects. RGB remains the default and retains original phase
  arithmetic. Nearest modes only use it for palette motion; dither remains discrete.
  Rainbow keeps generated HSV phase behavior but supports Oklab gradient segments;
  channel quantization ignores palette/interpolation as documented.
- Shared forward integer Oklab features are prepared once per evaluated gradient
  palette. Weighted biased Q10 coordinates round half up; inverse coefficients
  use Q15 Lab-to-LMS-root and Q12 LMS-to-linear-RGB matrices generated from the
  original Oklab coefficients. Signed rounds are half away from zero. Cubed roots
  produce Q16 LMS, linear RGB is clipped channel by channel, and encoding selects
  the nearest shared sRGB-table entry with upward ties. Generator assertions prove
  the inverse RGB sums fit signed 32-bit arithmetic at the conservative root bound.
  Exact endpoints and identical neighbor colors return authored bytes directly.
  This explicitly uses gamut clipping, not chroma-preserving gamut compression.
- Renderer controls reuse a padding word and the existing 16 packed feature entries.
  The parameter data remains 240 bytes in the existing 256-byte record; no extra
  pass, texture, binding, per-pixel allocation or readback. ASCII's palette evaluation
  explicitly retains RGB interpolation. Phase fractions for Oklab round to 1/65280.
- Actual GL/NVIDIA and Vulkan/llvmpipe tests passed independently with exact RGBA
  parity for uniform/nonuniform red-blue-white ramps and ember map/Bayer/blue-noise
  animation at times 0, .777s, 2s and 0. Explicit hardware 1080p/4K gradient validation
  also passed both sizes exactly, with persistent resources under 512MiB at 4K.
- Core checks: 324 passed/1 ignored. Independent black-to-white Oklab lightness
  formula agrees within one encoded byte across all 256 sample fractions; red-blue
  midpoint has a visible green component absent from RGB interpolation. Duplicate
  colors, authored endpoints and partial/hidden alpha are covered. CPU stylization
  checks: 7 passed. Legacy literal RGB ramp and six-stage threshold diagnostics pass
  after the shader change. Public API/typing checks after native rebuild: 179 passed.
  Workspace all-target/all-feature Clippy, schema and style checks passed; Clippy
  and style were refreshed successfully after the final alpha/4K fixtures.
- Inspected `target/stylization/oklab-interpolation-ramp-contact.png` and
  `target/stylization/palette-interpolation-video-contact.png`. New public example
  `target/stylized-showcase/smoke/dither-blue-noise-ember-nonuniform-oklab-cpu.mp4`
  uses `--stops 0 0.18 0.55 1 --interpolation oklab`; warm palette differences are
  subtle, while the red-blue ramp and grayscale lightness cases are distinctly
  different. Full temporal playback acceptance remains a Milestone 7 item.
- New `palette-interpolation-{smoke,1080p}` benchmark suites compare RGB/Oklab
  gradient mapping on the existing animated four-color video. Full 1080p capture
  completed on CPU and confirmed hardware GL; measured results follow.
- Milestones 5–7 remain incomplete. Next: entering-signal tone/detail controls and
  independent analysis resolution, followed by all-family customization and final
  visual/temporal/performance acceptance. No full verification gate is claimed here.

- Interpolation performance capture: one warmup/three samples, 90 frames at
  1920×1080/30fps, sequential CPU/hardware runs with frozen source inputs.
  CPU RGB **4671ms (4650–4742)** versus Oklab **6871ms (6790–6899)**:
  Oklab is approximately **47% slower end to end** on CPU and remains opt-in.
  Hardware GL RGB **3057ms (3007–3135)** versus Oklab **2948ms (2936–3151)**;
  ranges overlap and no GPU speed difference is established. These measurements
  include decoding/encoding; changed color output can change encoding work.
- Reports: `target/benchmark-results/palette-interpolation-{cpu,hardware}-20261009/suite.json`.
  Source SHA256 `7daa69089fc19752ba5962ec15cb5f6f4cec9fb987c6cbf4066351f7db8c1015`
  matches both captures and the final implementation inputs. Release executable
  SHA256 CPU `cc86939d55e0f1b04fbecef2f83d5f5ec434b98246bde6db99d684ad475f5126`,
  hardware `16c9e184b0fe00bda603172944f4d59e027d5130f2382ccc6028edb55a689d46`.
  Verified every sample's workload and backend; hardware selected NVIDIA GL
  DiscreteGpu throughout. 4K resource estimates remain 298,601,472 persistent
  and 398,134,272 staging bytes, excluding driver metadata/padding.


### Entering-signal exposure/gamma increment — 2026-10-09

- Added optional bindable `input_exposure`/`input_gamma` to PaletteMap and
  OrderedDither in canonical Rust/JSON, shared descriptors, compilation/evaluation,
  scalar dependency visits, Python high-level/advanced/generic APIs and schema.
  Defaults are 0/1. Constraints reuse ColorAdjust exposure (−8..8) and gamma
  (strictly positive through 8), including existing runtime constraints/floors.
- Active tone reuses the existing full-resolution ColorAdjust operation with
  neutral black/white points. Quantization reads the adjusted image while its
  final amount blends against retained original RGB/alpha. CPU and GPU use the
  existing OriginalAnd resource flow; GPU source is the processed image and
  auxiliary is the original. A spare parameter bit distinguishes this input;
  Oklab interpolation remains independent, and the record stays 240 bytes.
- Neutral compiled controls need no original/temporary preparation and retain
  one-pass output/metrics. Animated or bound controls conservatively reserve
  resources for active frames; amount zero still skips the effect. The public
  preparation requirement and pass-count functions remain const. No new shader
  pipeline, GPU readback, per-pixel allocation or frame history was introduced.
- Core: 325 passed/1 ignored; focused compile/evaluation test refreshed after
  preserving neutral pass counts. CPU stylization: 8 passed, including literal
  partial-alpha blend and hidden RGB. High-level/advanced/generic/typing API
  checks: 183 passed after native rebuild. All-target/all-feature workspace
  Clippy, schema generation/check and the focused schema test passed. Generic
  optional scalar defaults now use canonical tracks; schema defaults are track
  objects instead of scalar numbers. One earlier Python catalog run raced schema
  regeneration (182 passed/1 failed); refreshed schema and subsequent full
  focused run passed. The showcase formatting check was corrected with Ruff;
  style and 309 documentation-link checks then passed.
- Actual GL/NVIDIA and software Vulkan/llvmpipe independently passed the focused
  render test with exact RGBA parity: partial-alpha literal map/dither blending,
  plus map/Bayer8/blue-noise across monochrome/ember/ocean, animated gamma at
  0, .777s, 2s and repeated 0. The reference literal is original gray64, exposure1,
  nearest white, amount0.5 => gray160; blending from adjusted gray128 would be
  incorrect. These checks do not close all chromatic/tone/stack/mask combinations.
- Explicit hardware 1080p/4K blue-noise tone correctness passed exactly, including
  fractional amount. Persistent/staging estimates: 1080p **91,242,240/116,125,440**
  bytes; 4K **364,957,440/464,490,240** bytes, excluding driver metadata/padding.
  Active tone adds retained/prepared image storage versus the previous one-pass
  4K persistent estimate 298,601,472 bytes; the option remains opt-in.
- Inspected source/animated-tone contact sheets under
  `target/stylization/frames/gl/input-tone-{map,bayer,blue}-<palette>/contact.png`.
  Ember gamma visibly lifts midtones while preserving figure/moon/line contrast;
  contact panels are source, exposure0.5/gamma1, exposure0.5/gamma2. Inspected the
  public FFV1-source preview frame `target/stylization/input-tone-video-frame.png`;
  the oval subject remains visible. New public preview
  `target/stylized-showcase/smoke/dither-blue-noise-ember-tone-cpu.mp4` is 8 seconds,
  48 frames at 320×180, generated with `--input-exposure 0.5 --input-gamma 1.5`.
  Full playback/moving-subject acceptance remains open in Milestone 7.
- Captured `input-tone-1080p` sequentially on CPU/hardware GL, with frozen source
  and no concurrent render/test/build during timings: one warmup/three samples,
  90 frames at 1920×1080/30fps, FFV1 output. End-to-end wall medians/ranges (ms):
  CPU neutral blue noise **4734 (4596–4736)**, tone **4663 (4645–4827)**;
  hardware neutral **2945 (2936–3113)**, tone **3042 (2987–3102)**. Ranges overlap
  for both backends; no speed difference is established. Decoding/encoding and
  changed-output encoding cost are included; this does not isolate pass cost.
- Reports: `target/benchmark-results/input-tone-{cpu,hardware}-20261009/suite.json`.
  Both match final input source SHA256
  `28647e9bfdecf4b2dfbc08bb80e8ac5a3a7614a76b639d29f97e5b3fe8244c06`, dirty
  revision `1553308` plus this increment. Release executable SHA256 CPU
  `398c587d79259177e6ced27e3118d3b21594549328c7bdbfae3006f8101d43de`, hardware
  `b1aeddf19f1f8c6347dc54d75dc1d0411ef76ae74a36c5b41a418ad0fe159af2`.
  Every sample's resolution, frame count, workload and actual backend was checked;
  hardware selected GL, discrete_gpu, D3D12 NVIDIA GTX 1650 SUPER throughout.
- Milestones 5–7 remain incomplete. Next: entering-signal local contrast/detail,
  sharing Sharpen's existing Gaussian/unsharp passes before tone and quantization
  while keeping the same retained original; then independent input filtering/
  analysis resolution. Remaining effect families, expanded combination acceptance
  and complete final verification gates remain unexecuted, not passed.
