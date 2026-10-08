# Stylized video effects

Status: planned — product decisions finalized; implementation and low-level technical design remain.
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

### 1. Documentation and technical design

- [x] Add agent guidance, resumable plan convention and effect architecture page.
- [x] Expand effect extension guide and documentation navigation.
- [x] Record all 13 product decisions, including custom fonts, preset
  compatibility and explicit period semantics.
- [ ] Specify and review effect API schemas/parameters, full effect inventory,
  algorithms, RGB/alpha/border contracts, glyph handling and visual fixtures.
- [ ] Map each effect's CPU/WGPU pass/resource topology and identify justified
  extensions to prepared resources and renderer limits.

Completion: implementation-ready technical contracts without an unrelated
renderer rewrite.

### 2. Color and foundational primitives

- [ ] Reproduce the [palette-agnostic fine-detail dither target](../../development/stylization/dithered-palette-look.md),
  preserving outlines, midtone texture, shadow clarity and temporal stability
  with user-selected colors and adjustable fine/coarse pixel structure.
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
- [ ] Implement and verify equivalent CPU/WGPU paths, transparent/partial
  cell behavior, signal/keyframe support and applicable loop periods.
- [ ] Verify nonsequential frames, moving footage, typical 1080p throughput
  and 4K correctness/resources on available adapters.

Completion: advanced, deterministic cinematic ASCII usable on footage and
images, with no per-frame font/atlas rebuilds or GPU-to-CPU roundtrips.

### 4. Remaining stylization and reusable looks

- [ ] Implement halftone and CRT/analog styling on CPU/WGPU, including
  applicable periodic/animated parameters.
- [ ] Implement **both** horizontal and vertical bounded segmented pixel
  sorting with explicit thresholds, stable ties and resource-limit behavior;
  optional exotic variants may be deferred, not either required direction.
- [ ] Supply a small set of optional curated looks using current presets
  where valid and composable ordered effect recipes elsewhere. No parallel
  preset engine.
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
- [ ] Licenses/attribution for code, fonts, glyphs and showcase footage are
  preserved, with reproducible example inputs.
- [ ] Targeted tests plus `just check`, `just python-test`,
  `just docs-check`, `just wgpu-software` and
  hardware verification where available are reported accurately.

## Progress and verification

All **product choices** are finalized, and the current work consists solely
of documentation. Implementation, design-level parameter selection,
rendering and performance verification remain outstanding.
Next action: complete the two unchecked parts of Milestone 1 by deriving
concrete effect contracts/resource topologies from current code.

## Decisions and discoveries

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

Not complete; no effect code has been implemented in this documentation stage.
Proceed with the remaining *technical* design tasks, not more user preference
questions.
