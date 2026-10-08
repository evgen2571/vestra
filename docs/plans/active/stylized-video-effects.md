# Stylized video effects

Status: planned — documentation foundation prepared; code implementation pending.
Branch: `feat/stylized-video-effects`
Baseline: `6ec571f6282456d0a595fd9a1aa3ca1f2359c092`
Working instructions: [AGENTS.md](../../../AGENTS.md) and [PLANS.md](../../../PLANS.md)

## Objective

Create composable general-purpose cinematic effects for Vestra, with music
mix video backgrounds as one motivating use case. Target colored/monochrome
ASCII, pseudo-ASCII/hybrid overlays, animated palettes, dithering, halftone,
bounded pixel sorting and CRT/analog visual styling.

## Existing support and documentation

The project already has descriptor-driven effects, a canonical JSON model,
backend-neutral ordered effect passes, CPU/WGPU implementations, Python
wrappers, keyframe/audio-signal properties and benchmarks. Reuse existing
Bloom, Glow, ChromaticAberration and ColorAdjust; do not duplicate them.

See [effect extension](../../development/extending/effect.md),
[effect architecture](../../development/architecture/effect-pipeline.md),
[testing](../../development/testing.md),
[GPU validation](../../development/gpu-validation.md),
[performance](../../development/performance.md) and
[feature support](../../reference/feature-support.md).

Source landmarks: `crates/vestra-core/src/effect_definition.rs`,
`crates/vestra-core/src/project/model/effects.rs`,
`crates/vestra-core/src/plan/effect_passes.rs`,
`crates/vestra-render/src/cpu/effects.rs`,
`crates/vestra-render/src/wgpu/`,
`crates/vestra-render/src/shaders/effects/`,
`python/vestra/effects/`, `examples/effects/`.

## Scope and non-goals

Include parameter contracts, CPU/WGPU rendering, public Rust/Python/JSON
authoring, appropriate animation/signals, reusable shader resources,
performance/visual testing, examples and licensing provenance.

Exclude a new shader scripting engine, wholesale renderer rewrite, new
procedural video-source architecture, automatic seamless-loop generation,
changes to audio analysis and music-mix-specific APIs. Loop demonstration
may reuse existing layer/timeline facilities.

## Decisions to finalize before feature coding

| Area | Suggested starting contract; not final until reviewed |
| --- | --- |
| ASCII | cached glyph atlas, tile-based luminance plus optional edge analysis |
| Modes | glyph fill, edge-aware, blended pseudo-ASCII |
| Grid | output-pixel cell width/height, stable origin, partial-cell behavior |
| Color | monochrome, sampled source color, static or animated palette |
| Alpha | preserve source transparency; specify glyph coverage and blending |
| Stability | time-deterministic glyph selection; suppress distracting flicker |
| Placement | post-transform; layer/global wherever semantically valid |
| Sorting | bounded scanline/segment sorting with stable ties |
| Resources | bounded intermediates and persistent assets, no per-frame atlas upload |
| Parity | explicit visual tolerances and real-adapter test evidence |

Ordinary video cannot be assumed to contain depth/normals.
[AcerolaFX](https://github.com/GarrettGunnell/AcerolaFX) is a design reference,
not a direct WGSL drop-in; preserve its MIT notice if incorporating code.
License footage and glyph assets independently.

## Milestones

### 1. Documentation and architectural decisions

- [x] Add agent guidance, implementation plan convention and architecture reference.
- [x] Expand the effect integration guide, navigation and stale-release cleanup.
- [ ] Map concrete pass/resource requirements for each candidate effect.
- [ ] Decide effect names, types, defaults/ranges, animation/signal support,
  scopes, pixel/alpha/color semantics and deterministic visual fixtures.

Completion: interfaces/resource design are explicit and ready for code.

### 2. Foundational visual effects

- [ ] Implement configurable palette remapping and deterministic ordered dithering.
- [ ] Add generic intermediate/glyph resources only where justified.
- [ ] Implement each end-to-end (core model, compiler/evaluation, CPU/WGPU,
  API, schema, fixtures and docs).
- [ ] Record stable CPU/WGPU baseline performance and resource usage.

Completion: usable, fully integrated primitives.

### 3. Cinematic ASCII and pseudo-ASCII

- [ ] Implement atlas-based glyph selection with luminance/edge modes.
- [ ] Support colored/monochrome/hybrid modes and animated palette controls.
- [ ] Provide CPU/WGPU implementations and appropriate dynamic properties.
- [ ] Verify alpha, boundary cells, stability on moving video, 720p/1080p,
  and 4K when feasible.

Completion: visually stable video-capable ASCII, without per-frame
text-render-to-CPU work.

### 4. Other stylization

- [ ] Add configurable halftone and restrained CRT/analog styling.
- [ ] Add bounded deterministic pixel sorting; record or defer any expensive
  variants without blocking independent completed effects.
- [ ] Verify stack order, masks, global/clip scope and backend parity per effect.

Completion: diverse composable styles with documented constraints.

### 5. Final examples and verification

- [ ] Create small reproducible colored-ASCII, hybrid and alternative-effect
  examples with deterministic or appropriately licensed footage.
- [ ] Build a short repeating music-background demonstration using existing
  timeline APIs; do not imply a newly implemented seamless-loop feature.
- [ ] Run targeted/full Rust, Python, schema and documentation checks;
  validate actual WGPU frames, hardware when available, and benchmark deltas.
- [ ] Update `docs/guides/python/effects.md`, `docs/reference/effects.md`,
  `docs/reference/feature-support.md` and
  `examples/showcase/ASSETS.md` as necessary.
- [ ] Explicitly report blocked/unverified items and move plan to
  `completed/` only when acceptance is satisfied.

## Feature acceptance criteria

- [ ] Public Rust/Python/JSON contract, defaults, ranges, negative validation,
  serialization/schema and supported animation documented and tested.
- [ ] Deterministic CPU output and real WGPU output with justified tolerances.
- [ ] Transparent/semi-transparent, small-frame, partial-cell and resource
  boundary behavior covered.
- [ ] Effect stacking, clipping, groups, masks, mattes, layer/global scope
  and no-op semantics preserved.
- [ ] No unexpected GPU readbacks, per-frame atlas builds or full-frame
  allocations; benchmark against a reproducible baseline.
- [ ] Source/media/font licenses tracked and public examples can be reproduced.
- [ ] `just check`, `just python-test`, `just docs-check` and applicable
  `just wgpu-software` / `just wgpu-hardware` validations recorded.
  Unrun hardware tests are unverified, not passed.

## Progress and verification

Documentation milestone is underway. No new effect code or benchmark claims
are made by this plan. Next action: finalize Milestone 1 decisions and
resource topologies, then start end-to-end palette/dithering implementation.

## Decisions and discoveries

- 2026-10-08: Reuse current canonical descriptors and bounded effect-pass
  architecture, extending only for concrete feature requirements.
- 2026-10-08: Keep effects generally reusable, separate from the mix maker.
- 2026-10-08: Keep third-party shader and footage licensing independent;
  use the existing showcase asset ledger.

## Completion / handoff

Not implemented yet. Next: Milestone 1's open design decisions.
