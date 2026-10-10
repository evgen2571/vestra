---
name: effect-development
description: "Use when implementing or modifying Vestra visual effects, WGSL kernels, effect-pass planning, renderer resources, or their Rust/Python/JSON interfaces."
---

# Develop a Vestra visual effect

Follow [AGENTS.md](../../../AGENTS.md) and, for complex work,
[PLANS.md](../../../PLANS.md). Read the
[effect extension guide](../../../docs/development/extending/effect.md)
and [effect pipeline](../../../docs/development/architecture/effect-pipeline.md).
For stylization, use the [effect reference](../../../docs/reference/effects.md)
and [dithered-palette guide](../../../docs/development/stylization/dithered-palette-look.md).

## Workflow

1. **Specify behavior:** effect tag, clip/global scope, intrinsic stage,
   parameter types/defaults/ranges, ordering, timing/signals, coordinate
   units, color/alpha behavior, identity and failure modes.
2. **Find canonical ownership:** descriptors in
   `crates/vestra-core/src/effect_definition.rs`, tagged project model,
   validation, compile/evaluate and backend-neutral pass IR.
   Keep semantics in core; don't duplicate defaults across languages.
3. **Map resources before coding:** identify input/output passes, retained
   originals, live temporaries, prepared assets and GPU limits. Extend
   the bounded planner only for a demonstrated need.
4. **Implement end to end:** Rust CPU operation, WGSL kernels/parameters/
   WGPU dispatch, Rust SDK and Python authoring/lowering, and JSON schema.
   Ensure deterministic evaluation at arbitrary timestamps.
5. **Verify actual renders:** compare CPU/WGPU frames using
   [visual-regression](../visual-regression/SKILL.md). Test color/alpha,
   masks, groups, transforms, effect order, animation and resource failures.
6. **Complete public contracts:** validation errors, round trips, exports,
   typing, [effect reference](../../../docs/reference/effects.md), examples
   and [feature support](../../../docs/reference/feature-support.md).

## Verification

Use focused Rust/Python tests while implementing, then
`just schema-check`, `just check`, `just python-test`,
and suitable WGPU checks from [testing](../../../docs/development/testing.md).
For expensive effects, use [performance-benchmarking](../performance-benchmarking/SKILL.md).
A registered WGSL kernel is not completion if CPU parity, public API,
real output or documentation is missing.
