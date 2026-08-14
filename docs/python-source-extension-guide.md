# Adding a future source type

This guide describes how a future `Video`, `Text`, or `Rectangle` source would
fit into Vestra. These sources are intentionally not implemented by the Python
editing API v2 work. The point of the `Project` to `Composition` to `Layer` to
`Source` split is that adding one does not require a new editor model.

## The source boundary

A source owns source-specific data only. A future `Text` could hold its string,
font, size, and text layout. A future `Rectangle` could hold fill and stroke
values. A future `Video` could hold its asset, trim, and decode options.

Timing, z-order, visibility, opacity, blend mode, transform, effects, generic
keyframes, and signal bindings remain on `Layer`. `Composition` continues to
own sibling placements and transitions. Audio remains a project timeline.

## Required implementation steps

1. Add a new `Source` subclass in `vestra.sources` with validated,
   source-specific values. Keep it copy-safe when a source is placed more than
   once.
2. Add one lowering registration in `vestra.lowering` with a handler and a
   `SourceCapabilities` value. The registration is the single dispatch point
   for asset registration, canonical representation, and capability facts.
3. Add the canonical/native compiler representation for the source. The
   lowering handler should produce that representation through the existing
   `ProjectBuilder` path rather than writing ad hoc JSON from `Project` or
   `Layer`.
4. Implement matching CPU and WGPU renderer semantics in the native crates.
   Both backends must agree on timing, transforms, effects, alpha, and any
   source-specific values that the source promises.
5. Add native validation, preparation, and rendering tests. Add high-level
   lowering, snapshot, ownership, capability, and frame tests. Include asset
   deduplication where the source uses external files.
6. Update the Python API guide, capability notes, parity audit, and a runnable
   example. Document any intentional backend or nesting limitation.

The source handler may use a capability adapter when the native source lacks a
direct transform or transition endpoint. The adapter must preserve local time,
IDs, effects, blend, and opacity. It should be added only after semantic and
performance tests justify it.

## What does not change

Adding `Video`, `Text`, or `Rectangle` should not require changing:

- `Project` construction, snapshot, validation, preparation, or rendering.
- `Composition.add`, `Composition.group`, ownership, or local timing rules.
- `Layer` presentation, `EffectStack`, transitions, or root overlays.
- Typed properties, keyframes, easing, or signal binding.
- Audio tracks, clips, and master signal factories.
- The prepared frame and render-result lifecycle.

Those APIs depend on the `Source` contract and centralized lowering, not on a
closed union of every source class. If a proposed source needs edits across
these objects, first check whether the new behavior belongs in source
capabilities or native lowering instead.

## Capability examples

The central capability record currently describes facts such as direct
transforms, direct transition endpoints, sizing, crop, presets, and audio
requirements. A source can report that it supports a direct transform, or the
lowering layer can place it inside a presentation group when that is the
validated native equivalent.

Transitions are currently root-only. Direct native endpoints are `Image` and
`Group`; the high-level adapter can support current `Color`, `ParticleSystem`,
and `Spectrum2D` cases where the lowering semantics are valid. This rule is a
capability decision, not a reason to hardcode source names throughout the
editor.
