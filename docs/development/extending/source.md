# Add a source

Add a source when existing visual values cannot express the content. Decide
whether it is asset-backed, procedural, or a nested composition: that determines
what preflight prepares and which timing rules core must own. Read
[project to plan](../architecture/project-to-plan.md) before changing those boundaries.

Follow [Spectrum2D's canonical model](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-core/src/project/model/spectrum2d.rs)
and [CPU implementation](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-cpu/src/cpu/spectrum2d.rs)
for a procedural source. Use an existing Image/Video variant in
[visual.rs](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-core/src/project/model/visual.rs)
for an asset-backed source; Spectrum2D's audio-analysis requirements are not a
default for other sources.

## Find the owners

| Responsibility | Location |
| --- | --- |
| Canonical values and semantic rules | `crates/vestra-core/src/project/model/`, `validation/visual/` |
| Compilation, timing and evaluated sources | `crates/vestra-core/src/plan/`; see the [core source map](../architecture/project-to-plan.md#core-source-ownership) |
| External paths, probing and preparation | `crates/vestra/src/`, `crates/vestra-media/src/`; shared prepared rendering assets in `vestra-render-common` |
| Pixel execution and capability decisions | `crates/vestra-render-cpu/src/cpu/`, `crates/vestra-render-wgpu/src/wgpu/` and `src/shaders/` |
| Python value, lowering and exports | `python/vestra/sources/`, `python/vestra/lowering.py`, package exports; native methods/stubs only when needed |

## Implement in order

1. Add the tagged canonical representation and source-specific values in core.
   Update schema serialization/template support and semantic validation. Keep
   path resolution and media probing out of core.
2. Extend compilation and evaluated source data. Put source-local timing,
   transition endpoint eligibility and nested composition rules here. A renderer
   receives resolved/evaluated values, not raw public JSON.
3. Extend preflight and preparation for external files, dimensions, decoded
   resources or analysis. Resolve asset paths from the `Project` base directory
   at the SDK/media boundary and share immutable prepared assets where possible.
4. Implement CPU dispatch/resource reuse, then WGPU dispatch, uploads and
   readback-compatible output. Add only the capability each backend really has.
5. Add the high-level `vestra.sources` value and lowering. Add advanced
   `ProjectBuilder` support if it belongs in canonical authoring. Export the
   public name and update `_native.pyi` only for native binding changes.

## Verify and document

Use focused model/schema-validation and high-level lowering tests, then actual
CPU and WGPU frames. Test nested compositions where relevant and effect/transition
attachments for each claimed capability. Follow [Testing](../testing.md) for
commands and [GPU validation](../gpu-validation.md) for separate software and
hardware evidence. Update the source reference and
[feature support](../../reference/feature-support.md).

The change is ready when:

- The canonical tag, schema, serialization and semantic diagnostics agree.
- Compilation/evaluation and all external-resource ownership are covered.
- Python authoring, lowering, native stubs where changed, and exports agree.
- Each backend has tested support or an explicit limitation.
- The reference states only demonstrated capabilities. A source that renders
  does not thereby support direct transforms or transition endpoints.
