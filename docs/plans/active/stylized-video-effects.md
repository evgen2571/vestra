# Stylized video effects

Status: in progress — all effect families implemented; comprehensive validation, hardware quality and performance acceptance underway.
Branch: `feat/stylized-video-effects`
Baseline: `6ec571f6282456d0a595fd9a1aa3ca1f2359c092`
Decisions finalized: 2026-10-08
Working instructions: [AGENTS.md](../../../AGENTS.md) and [PLANS.md](../../../PLANS.md)

## Objective

Deliver a general-purpose, composable suite of cinematic video stylization
effects in Vestra, with music-mix backgrounds as an example rather than a
special-case API. The branch includes **all** planned families: cinematic ASCII,
pseudo-ASCII/hybrid rendering, palette mapping and animated/rainbow color,
ordered dithering, halftone, bounded horizontal/vertical pixel sorting, and
CRT/analog styling. Reuse existing Bloom, Glow, ChromaticAberration and
ColorAdjust instead of duplicating them.

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

### 5. Showcase, regression and documentation

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

All **product choices** are finalized. Milestone 1 technical contracts are
recorded in this plan. The branch contains an initial PaletteMap/OrderedDither
implementation across core, CPU, WGPU and public authoring, with focused
tests, examples and benchmark suite definitions. Milestone 2 remains unchecked:
visual quality, complete validation, actual 1080p/4K measurements and hardware
evidence have not been confirmed by this documentation update.

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

Not complete. All planned families and public interfaces are implemented.
Finish hardware quality/resource checks, comprehensive checks and serial
1080p measurements before closing acceptance and moving this plan.

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
