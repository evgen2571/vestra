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
   `crates/vestra-core/src/effect_definition/catalog_entries.rs` and
   `crates/vestra-core/src/project/model/effects.rs`. Centralize
   scalar constraints in `effect_definition/properties.rs`; descriptor builders
   in `effect_definition/descriptors.rs` feed validation and schema generation.
   Do not copy
   defaults into independent Python, CPU and WGSL tables.
3. **Compile and evaluate.** Extend the core effect compiler/evaluator
   and ordered pass IR in `crates/vestra-core/src/plan/`.
   Preserve frame-time semantics, effect order and scope.
4. **Design resource flow.** For multi-pass operations, map live inputs,
   outputs, original-image retention and temporary resources before
   extending the bounded working set. Check GPU preparation requirements,
   aliases, device limits and failure paths. Add shared operation-to-kernel
   mapping/requirements in `vestra-render-common`; keep capability policy in
   the respective backend's `support.rs`.
5. **Implement CPU.** Extend `crates/vestra-render-cpu/src/cpu/effects.rs`
   and reusable surfaces/resources. Define border, alpha, color and
   deterministic time behavior.
6. **Implement WGPU.** Add WGSL kernels under
   `crates/vestra-render-wgpu/src/shaders/effects/`, shader/kernel registration,
   parameter layouts, pipeline preparation, frame planning and execution.
   Match Rust/WGSL layouts and avoid unneeded allocations/readbacks.
7. **Expose the authoring API.** Add Python-specific names, argument order and
   compatibility overrides in `scripts/effects/python_metadata.py`, then run
   `just gen-effects`. Extend handwritten adapters, lowering and public exports
   where the new behavior requires them; update native stubs only where needed.
   Keep Rust SDK and canonical JSON contracts coherent.
8. **Validate and document.** Update the
   [effect reference](../../reference/effects.md),
   [Python guide](../../guides/python/effects.md) and
   [support matrix](../../reference/feature-support.md) for verified behavior.

## Generated declarations

The semantic catalog in `effect_definition/catalog_entries.rs` is shared by the
runtime catalog macro and the development-only `effect_metadata` example.
Parameter kinds, constraints, wire defaults, scope and execution facts come from
that catalog and its existing descriptor builders. Python argument order,
convenience defaults, enum names and inheritance belong to
`scripts/effects/python_metadata.py`, keyed by canonical effect ID. The generator
derives declarations from parameter kinds and uses explicit overrides for
Python behavior that differs from the wire contract.

Run these commands in the development environment after changing metadata:

```bash
just gen-effects
just gen-effects-check
```

Generation updates marked sections in the eight high-level effect modules,
their `.pyi` files, the 21 existing typed collection helpers, the visual branches
of `schemas/project.schema.json`, and the
[parameter reference](../../reference/effect-parameters.md). Check mode reports
stale artifacts without writing; regeneration preserves unchanged files. Commit
the generated output with its metadata change. Package users do not run either
command.

The generated reference owns factual parameter tables. Keep handwritten effect
descriptions for rendering semantics, combined constraints and examples.

The pure visual schema builder lives in `scripts/effects/schema.rs`, shared by
the CLI schema command and the core-only exporter. This keeps schema formatting
and Python generation out of core's runtime API while reusing the established
catalog and schema logic. The exporter needs `vestra-core`; it does not compile
CPU/WGPU renderers. Rendering does not parse generation metadata.

Generated constructors, property accessors and helpers call the existing
handwritten authoring code. Keep validation timing, ownership, scalar versus
plain tracks, point/font/palette conversion and lowering in those adapters.
`Brightness`/`Contrast`/`Saturation` retain `_AmountEffect` inheritance,
`OrderedDither` retains `PaletteMap` inheritance, and `PseudoAscii` remains a
handwritten `Ascii` adapter. Advanced typed classes and generic `add_effect`
remain handwritten because their construction and validation behavior differ.
See the [effect reference](../../reference/effects.md) for catalog counts and the
legacy Ascii schema/parser exception.

For a metadata-only correction, changing an existing parameter's units in the
Python metadata updates the generated reference without changing its constructor
or rendering. For a default shared with JSON, `PaletteMap.levels` has catalog
default `4` and no Python override. Changing that default to `8` in an isolated
checkout propagates to its constructor, helper, stub, schema and reference.
Keep such demonstrations out of shipped contracts.

`test_canonical_kind_and_default_propagate_without_signature_copies` in
`tests/test_effect_generation.py` verifies declarations, helpers, stubs and
reference output using scratch metadata; core exporter tests verify schema
synchronization. Intentional public contract changes still need canonical model,
validation and implementation work.

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
See the [effect recipes](../../reference/effects.md).

## Acceptance checklist

- Descriptor range/default/type/scope checks, serialization,
  `just gen-effects-check`, generated schema and negative semantic-validation tests.
- Compilation/evaluation and effect ordering, no-op values and supported
  keyframe/signal bindings.
- Deterministic CPU frames, parsed WGSL, actual WGPU output, and
  justified parity tolerances. Verify alpha, masks, mattes, groups and
  stacking with other effects.
- Real moving-footage frames where temporal effects are involved.
  Treat software-WGPU verification and hardware-GPU verification separately.
- Representative benchmark/resource-limit checks for costly effects.
- Python/Rust/JSON API use, examples, documentation, and asset licenses.

Use focused `cargo test -p vestra-core`, `cargo test -p vestra-render-common`,
`cargo test -p vestra-render-cpu` or `cargo test -p vestra-render-wgpu` during
development. Cross-backend tests run through `cargo test -p vestra-render
--test parity --all-features`; preserve the independent backend dependency
graphs. Then run applicable `just check`,
`just python-test`, `just docs-check`, `just wgpu-software`, and
`just wgpu-hardware` when real hardware is available. See
[Testing](../testing.md), [GPU validation](../gpu-validation.md) and
[Performance](../performance.md) for prerequisites. A shader compile test or
dispatch arm alone does not establish correct visual output.
