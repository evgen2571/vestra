# Effect pipeline and renderer resource constraints

This is the architecture reference for visual effects, including advanced
multi-pass shaders. Use [Add an effect](../extending/effect.md) as the
implementation checklist, and verify precise behavior against current source.

## Ownership and data flow

```text
Python / Rust / canonical JSON authoring
  -> core effect descriptors, model and validation
  -> core compilation and frame-time evaluation
  -> backend-neutral effect-pass plan
  -> CPU effect surface execution | WGPU frame plan, parameters, WGSL
  -> composition / masks / mattes / final output
```

| Responsibility | Implementation |
| --- | --- |
| Canonical descriptors / validation | `crates/vestra-core/src/effect_definition.rs` |
| Canonical effects | `crates/vestra-core/src/project/model/effects.rs` |
| Compile/evaluate | `crates/vestra-core/src/plan/compiler/effects.rs`, `plan/evaluation/effects.rs` |
| Logical operations and resource needs | `crates/vestra-core/src/plan/effect_passes.rs` |
| CPU | `crates/vestra-render/src/cpu/effects.rs` and surface pools |
| WGPU | `crates/vestra-render/src/wgpu/frame_plan.rs`, `parameters/effects.rs`, `pipeline.rs`, `executor.rs` |
| Shader source | `crates/vestra-render/src/shaders/effects/` and kernel registration |
| High-level Python | `python/vestra/effects/`, authoring and lowering |
| Generated project schema | `schemas/project.schema.json` |

Core owns authored/evaluated semantics; backends must not reinterpret defaults.

## Ordering and spaces

Layer presentation order is:

```text
source sizing/crop -> pre-transform effects -> layer transform
-> post-transform effects -> masks -> track matte -> opacity/blend
```

Motion Tile is currently the sole pre-transform effect. Ordinary image
stylization belongs after transform, respecting effect declaration order.
Global `post_effects` are applied to the final composition. Nested groups
preserve local timing. Define legal layer/global scopes, coordinate space,
units, pixel-center conventions and handling of output borders/partial cells.

## Bounded working set

The core pass IR currently represents `Original`, `Current`,
`Temporary0` and `Temporary1`, together with explicit pass dependencies
and retained-original requirements. WGPU maps these to fixed slots including
`Layer`, `EffectA`, `EffectB` and optional `Auxiliary`.
Its working textures currently use encoded, straight-alpha `Rgba8Unorm`.

This is **not** an arbitrary render graph. Before an advanced effect:

1. Write its pass sequence, live resources and original-image dependencies.
2. Separate frame-sized intermediates, lower-resolution analysis, persistent
   glyph atlases and shader uniforms. Check for read/write aliasing.
3. Confirm preparation-time requirements and device/texture/buffer limits.
   Add clear failure diagnostics where resources are unsupported.
4. Reuse pipelines, static textures, atlases, buffers and staging resources;
   don't create full-frame allocations or read back to CPU per frame.
5. Respect GPU completion before resource reuse. Extend the IR/slots only
   for a demonstrated need, with resource-limit and reuse regression tests.

## Shader, alpha, color and time contracts

CPU and WGPU must consume the same evaluated values in the same pass order.
The WGPU path uses WGSL and packed Rust parameter records: match layouts,
bindings, workgroup dimensions, edge dispatch and shader registration.
A shader parse test is not a rendered-frame validation.

Encoded byte-space RGBA and straight alpha are existing pipeline facts,
not a promise that luminance and blending happen in linear light. Specify
luminance computation, any color-space conversion, thresholds, quantization,
alpha behavior and numeric tolerances. Test fully transparent inputs with
hidden RGB, semi-transparent edges, stacking, masks/mattes and identity
settings. Never unintentionally fill the transparent backdrop.

Time-dependent effects must derive from evaluated project time,
keyframes/signals and explicit seeds, not wall-clock time or nondeterministic
frame state. Persistent caches must key all inputs affecting their output.

## Stylization-specific constraints

ASCII/pseudo-ASCII requires a defined glyph selection rule, legally usable
atlas/font, cell width/height and aspect ratio, grid anchoring, edge/fill
modes, antialiasing, source/palette colors, alpha and temporal stability.
Ordinary video does not provide depth/normal buffers, unlike some ReShade
game shaders. Use video-safe luminance/edge analysis. Prefer prepared glyph
resources and per-cell/lower-resolution analysis when justified.

For dithering define the threshold matrix, grid anchor and deterministic
noise; for halftone define dot/channel geometry; for pixel sorting define
bounded segments, comparator, stable ties and GPU memory/time cost.
Unbounded arbitrary full-frame sorting is not a reasonable default.

## Verification

Require canonical serialization/validation/schema, authoring interfaces,
CPU frame tests, WGSL parsing and actual WGPU frame tests, animation,
alpha/mask/group integration, deterministic visual comparisons, real video
and performance/resource limits. Distinguish software adapter correctness
from hardware validation. Use [Testing](../testing.md),
[GPU validation](../gpu-validation.md), [Performance](../performance.md) and
[effect reference](../../reference/effects.md). Document licenses as in
[showcase assets](../../../examples/showcase/ASSETS.md).
