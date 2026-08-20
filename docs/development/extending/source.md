# Add a source

Adding a visual source crosses the canonical model, compiler and both renderer paths. Start by deciding whether the source is an asset-backed value, a procedural value, or a nested composition-like value. That choice determines what preflight prepares and which timing/capability rules core must own. Use existing variants in `crates/vestra-core/src/project/model/visual.rs`, `crates/vestra-core/src/plan*`, `crates/vestra-render/src/cpu` and `crates/vestra-render/src/wgpu` as the current route map.

1. Add the tagged canonical source representation and any source-specific values in `vestra-core` project model. Update schema serialization/template support and semantic validation. Keep path resolution and media probing out of core.
2. Extend plan compilation and evaluated source representation. Put source-local timing, transition endpoint eligibility and nested composition rules here. A renderer should receive resolved/evaluated values, not raw public JSON.
3. Extend preflight and resource preparation for external files, dimensions, decoded resources or analysis. Asset paths resolve from the `Project` base directory at this SDK/media boundary.
4. Implement CPU dispatch and resource reuse, then WGPU dispatch, resource uploads and readback-compatible output. Add only the capability each backend really has.
5. Add the high-level `vestra.sources` value and lowering in `python/vestra/lowering.py`; add advanced `ProjectBuilder` support if it belongs in canonical authoring. Export the public name and update `_native.pyi` only for native bindings.

Complete the change with focused model/schema validation tests, high-level lowering tests, CPU and WGPU tests, nested-composition tests where relevant, and effect/transition tests for each claimed attachment capability. Then update the exact source page and [feature support](../../reference/feature-support.md). Do not infer direct transforms or transition endpoints from the fact that a source renders.

Checklist:

- Canonical tag, schema, serialization and semantic diagnostics are implemented.
- Compiler/evaluator has a renderer-independent variant.
- Preflight/resource ownership handles every external dependency.
- CPU and WGPU support or explicit limitation is tested.
- Python authoring/lowering and public exports are coherent.
- Reference and support matrix state only demonstrated capability.
