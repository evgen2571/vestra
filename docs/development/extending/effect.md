# Add a visual effect

The visual effect API is descriptor-backed. Before adding a multi-pass
shader or changing working resources, read the
[effect pipeline architecture](../architecture/effect-pipeline.md).

## Implementation route

1. **Specify public behavior.** Define effect identity, supported attachment
   scope, pre-/post-transform stage, ordering, parameter units, defaults,
   bounds, time/signal support, color and alpha behavior, and identity values.
   For larger features record decisions in an [execution plan](../../../PLANS.md).
2. **Define the canonical contract.** Update
   `crates/vestra-core/src/effect_definition.rs` and
   `crates/vestra-core/src/project/model/effects.rs`. Centralize
   validation and schema generation through descriptors. Do not copy
   defaults into independent Python, CPU and WGSL tables.
3. **Compile and evaluate.** Extend the core effect compiler/evaluator
   and ordered pass IR in `crates/vestra-core/src/plan/`.
   Preserve frame-time semantics, effect order and scope.
4. **Design resource flow.** For multi-pass operations, map live inputs,
   outputs, original-image retention and temporary resources before
   extending the bounded working set. Check GPU preparation requirements,
   aliases, device limits and failure paths.
5. **Implement CPU.** Extend `crates/vestra-render/src/cpu/effects.rs`
   and reusable surfaces/resources. Define border, alpha, color and
   deterministic time behavior.
6. **Implement WGPU.** Add WGSL kernels under
   `crates/vestra-render/src/shaders/effects/`, shader/kernel registration,
   parameter layouts, pipeline preparation, frame planning and execution.
   Match Rust/WGSL layouts and avoid unneeded allocations/readbacks.
7. **Expose the authoring API.** Extend `python/vestra/effects/`,
   lowering and public exports; update native stubs only where needed.
   Keep Rust SDK and canonical JSON contracts coherent.
8. **Validate and document.** Update the
   [effect reference](../../reference/effects.md),
   [Python guide](../../guides/python/effects.md) and
   [support matrix](../../reference/feature-support.md) for verified behavior.

## Additional contracts for font and time-driven effects

For glyph effects, specify a built-in licensed glyph resource and portable
custom character/font authoring. Resolve font paths in preflight, validate
glyph coverage and invalid resources, prepare/cache an atlas, and ensure both
renderers render the same selections without rebuilding it per frame.

For periodic effects, define a positive finite loop duration in seconds,
evaluation-time origin, phase wrap and how keyframes/signals combine with
generated motion. Test arbitrary/nonsequential frame requests and timestamps
separated by a full period. Do not promise source video/audio loops.

When exposing convenient looks, remember existing cinematic presets are
Image-only and one per image layer. Use composable effect recipes for video
and global effects instead of inventing a second incompatible preset engine.
See the [completed feature decisions](../../plans/completed/stylized-video-effects.md).

## Acceptance checklist

- Descriptor range/default/type/scope checks, serialization, generated schema,
  and negative semantic-validation tests.
- Compilation/evaluation and effect ordering, no-op values and supported
  keyframe/signal bindings.
- Deterministic CPU frames, parsed WGSL, actual WGPU output, and
  justified parity tolerances. Verify alpha, masks, mattes, groups and
  stacking with other effects.
- Real moving-footage frames where temporal effects are involved.
  Treat software-WGPU verification and hardware-GPU verification separately.
- Representative benchmark/resource-limit checks for costly effects.
- Python/Rust/JSON API use, examples, documentation, and asset licenses.

Use `cargo test -p vestra-core` / `cargo test -p vestra-render` with
focused filters during development. Then run applicable `just check`,
`just python-test`, `just docs-check`, `just wgpu-software`, and
`just wgpu-hardware` when real hardware is available. See
[Testing](../testing.md), [GPU validation](../gpu-validation.md) and
[Performance](../performance.md) for prerequisites. A shader compile test or
dispatch arm alone does not establish correct visual output.
