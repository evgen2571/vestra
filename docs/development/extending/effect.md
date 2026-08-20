# Add an effect

Visual effect facts belong in the descriptor catalog, not in duplicated Python, schema and renderer tables. The core descriptors in `crates/vestra-core/src/effect_definition.rs` define effect identity, parameter kinds, defaults, ranges, attachment scope and metadata used by canonical validation and schema generation. Start there and use existing effects in `crates/vestra-core/src/project/model/effects.rs`, `vestra-render/src/effects.rs`, `cpu/effects.rs` and `wgpu/parameters/effects.rs` as integration examples.

Add the descriptor and its canonical representation first. Give each parameter its public type, default, finite/range rule, unit and whether it is a scalar property that can animate or bind signals. Add validation through the descriptor route, then make compilation/evaluation preserve ordered effect attachment. Do not recreate defaults in Python or a shader by hand.

Implement the CPU effect pass and WGPU parameter/pipeline/pass path separately. Both need to consume the same evaluated parameter values and respect layer versus post/global scope. A dispatch arm alone is not parity evidence. Add a Python class under `python/vestra/effects`, export it, lower it to the descriptor form and keep authored argument validation aligned with the canonical contract.

Test defaults and serialization, bad ranges/types, attachment scope, keyframes/signals where allowed, stacking order, CPU output and WGPU output. Regenerate/check the schema when the descriptor affects it. Finally update [Effects](../../reference/effects.md) with the exact Python parameters, canonical names and cautious backend support claim.
