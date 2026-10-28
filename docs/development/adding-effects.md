# Add a visual effect

Define an effect once, keep its semantic decisions in its own Rust module, and
register a kernel only when existing operations cannot implement it. Start with
[Vignette](../../crates/vestra-core/src/effects/vignette.rs),
[Glow](../../crates/vestra-core/src/effects/glow.rs), or
[Camera Shake](../../crates/vestra-core/src/effects/camera_shake.rs).

Read the [effect pipeline](architecture/effect-pipeline.md) before changing
resources. The [effect reference](../reference/effects.md) owns coordinate,
alpha, color and timing contracts.

## Choose the route

| Kind | Core work | Renderer work |
| --- | --- | --- |
| Ordinary image processing, like Vignette | Declaration, semantics, logical operation | CPU algorithm; WGPU shader/packing; backend registrations |
| Composite, like Glow/Bloom | Declaration and passes using existing operations | None when existing kernels suffice |
| Transform-only, like Camera Shake | Declaration, local-time evaluation and `Transform` hook | No image-processing operation, kernel, shader or uniform |
| Source sampling, like Motion Tile | Pre-transform semantics and sampling parameters | Source-specific CPU/WGPU adapters |
| Prepared-resource processing, like ASCII | Resource validation and multi-stage topology | Preparation, analysis/resolve kernels and resources |

An effect can use several operations; several operations can share a kernel.
Gaussian's two directions share one kernel, and Glow reuses extraction,
Gaussian and composition. `ColourTransform` is internal compiled/evaluated data,
without an authored public effect.

For a small ordinary effect with its CPU algorithm in the existing effects
file, expect eight maintained files: catalog, semantic module, logical operation,
common kernel declaration, CPU implementation/dispatch, WGPU registry, WGPU
record/packing and WGSL. A composite or transform effect using existing renderer
behavior needs only the catalog and semantic module. Tests and generated output
are additional; specialized resources can require more implementation files.

## Declare fields and parameters

Edit [catalog_entries.rs](../../crates/vestra-core/src/effect_definition/catalog_entries.rs).
Each entry owns the Rust variant, semantic module, canonical JSON `id`, class,
scope, stage, temporal policy, conservative pass estimate and original-retention
policy. It also contains:

- Native `authored`, `compiled` and `evaluated` fields, including actual serde
  attributes. Keep their types explicit: a color string becomes bytes; a scalar
  property becomes a compiled property and then an evaluated number.
- Scalar/plain-track field-to-target associations and their constraints.
- Ordered parameter descriptors, defaults and optional semantic hooks.

Macros derive whole enums in their existing defining modules, wire tags,
kind/class/stage projections, target identities/names, typed visitors, generic
track normalization/remapping, module declarations and semantic dispatch.
Preserve defaults, flattening, optional/null behavior and unknown-field rejection.

Use Vignette's complete entry as the example. A scalar association is:

```rust
amount => VignetteAmount = 59: {
    ScalarPropertyDefinition {
        runtime_constraint: RuntimeRange { min: 0.0, max: 1.0 },
        authored_validation: Range {
            min: Some(0.0), max: Some(1.0),
            min_exclusive: false, max_exclusive: false,
        },
    }
}
```

Its descriptor uses `EffectParameterDescriptor::scalar(ScalarPropertyTarget::VignetteAmount)`.
Plain tracks use the corresponding `PlainTrackTarget` association. Share a
constraint definition when properties deliberately follow the same rule.

Existing target discriminants preserve observable Rust enum casts. Allocate an
unused value across the **whole** generated enum: `RotationDegrees = 72` is a
non-effect target, so scanning catalog entries alone misses that reserved value.
Duplicate discriminants fail compilation. Kernel bits have separate numbering.

Ordinary new tracks need no traversal overrides. The ten existing palette input
fields marked `#[preserve_authored_track]` retain their historical transition and
normalization behavior; do not copy that marker to a new effect by default.
For typed point/timing projections, use `CompiledPointProperty`/`ActiveInterval`
or their canonical `crate::plan::`/`crate::project::` paths, with field attributes
as needed. Arbitrary type aliases are outside this declaration grammar.

## Implement semantic decisions

Add `crates/vestra-core/src/effects/<module>.rs`; the declaration includes it
automatically. Follow the closest existing module:

- `validate` owns ordered scalar/plain/point validation, colors, integers and
  cross-field rules. Reuse `validation/` helpers and deliberate diagnostic
  codes, messages and pointers.
- `compile` compiles tracks/modifiers with their declared constraints and
  prepares immutable backend-neutral values.
- `evaluate` uses owner-local time, root project time, prepared signals and
  explicit seeds. Results must not depend on frame request order.
- `static_identity` and `evaluated_identity` distinguish a removable static
  effect from an identity on one frame. Account for animation/modifiers.
- `compiled_passes` and `evaluated_passes` describe conservative reachable work
  and the actual frame's ordered operations.

Simple effects need no default forwarding functions. Add `Dynamic`, `Modifiers`,
`PassCount` or `Requirements` to `hooks` only when overriding the ordinary
property/catalog behavior, and implement the corresponding `extra_dynamic`,
`extra_modifiers`, `pass_count` or `extra_requirements` function. Palette Map
demonstrates time/resource/pass-count overrides. Missing declared overrides
fail compilation; omitted overrides use false/catalog defaults directly.
For animated point properties, follow Radial Blur's `Dynamic` and `Modifiers`
hooks: the ordinary dependency traversal covers scalar and plain tracks, while
point modifiers and authored point keyframes need these explicit policies.

These functions are statically dispatched; missing required functions fail
compilation. Registration macros do not infer algorithms from parameter metadata.

The `Assets` hook calls local `validate_assets`; ASCII validates font references
there. `Transform` calls `apply_transform` after ordinary transform evaluation
on supported sources; Camera Shake owns its transform algorithm call. `Motion`
calls `resolve_motion` with an immutable layer/time/frame context; Motion Blur
samples displacement there. Basic colour entries implement `compose_colour`.
These routes need no new cases in the central frame evaluator.
Direct transform presentation currently supports Image, Shape, Text and Group;
the hook does not broaden the existing source-support policy.

## Declare passes and resources

Use `Original`, `Current`, `Temporary0`, `Temporary1` and explicit
`EffectPassInputs`. Map live inputs and outputs before coding. Preserve
retained originals and secondary inputs across both preparation and execution;
do not read a dead temporary or overwrite a still-live input.

Vignette emits one pass. Glow emits highlight extraction, horizontal/vertical
Gaussian passes and additive composition using a retained original; a new
Glow-like effect needs no artificial kernel. ASCII has analysis/resolve passes
and a prepared atlas with its own lifetime and shared shader fragments.

For genuinely new work, add typed evaluated fields to `EffectOperation` in
[plan/effect_passes/mod.rs](../../crates/vestra-core/src/plan/effect_passes/mod.rs).
Keep this IR backend-neutral. Conservative topology must include operations
reachable on any active frame, including when a sampled frame is an identity.

## Register a kernel and CPU behavior

Add one row in [render-common/kernel.rs](../../crates/vestra-render-common/src/kernel.rs):

```rust
Vignette = 7 => [Vignette],
```

The row derives the kernel enum, inventory, private bit and exhaustive operation
mapping. Preserve existing order/bits. Append a new kernel with an unused bit
below 64; bits are independent of enum discriminants. Gaussian demonstrates
mapping several operations to one kernel.

In [CPU effects](../../crates/vestra-render-cpu/src/cpu/effects.rs), add the
algorithm and one `cpu_effect_kernels!` row binding explicit support to typed
dispatch. Reuse source/secondary/target surfaces, prepared atlases and pools.
A dedicated algorithm module additionally needs its ordinary Rust declaration.
Unsupported kernels need explicit `unsupported` entries and operation patterns;
missing capability/dispatch cases fail compilation. Preserve alpha, Gaussian
direction adapters, profiling and temporary liveness.

## Implement WGPU behavior

An ordinary new WGPU kernel has three maintained locations:

1. The WGSL algorithm under `crates/vestra-render-wgpu/src/shaders/effects/`.
2. An explicit `effect_parameters!` record and operation packing in
   [parameters/effects.rs](../../crates/vestra-render-wgpu/src/wgpu/parameters/effects.rs).
3. One [registry.rs](../../crates/vestra-render-wgpu/src/wgpu/registry.rs) row
   declaring support, parameter type, exact shader label and ordered static
   source fragments.

That row derives shader projections, parameter variants/identities, typed
uploads, maximum record size and ABI-test coverage. Preparation retains common
kernel order. No separate shader, parameter-import or upload inventories are
needed; shaders are included statically.

Record fields and packing stay explicit: public descriptors cannot specify GPU
layout or numerical conversion. `effect_parameters!` supplies aligned C layout
and structural assertions. Normally match Rust/WGSL field names; Zoom Blur
demonstrates explicit vector-component mapping. Every registered record receives
size/alignment, scalar-kind, named-offset, WGSL and typed-upload checks. These
do not prove algorithms or every packed value. Keep pixel tests and the runtime
kernel-mismatch guard.

Use the WGPU `unsupported` section when no implementation exists. A registered
implementation can also explicitly decline support. Registration, backend
policy, device features/limits and resource preparation remain separate checks.
Preserve `RENDER-UNSUPPORTED-KERNEL`, explicit-backend errors and `Auto` fallback.

## Generate the public surface

```bash
just gen-effects
just gen-effects-check
```

The core-only exporter supplies descriptors/schema facts. Generation derives
high-level classes/stubs, advanced typed handles, enums, collection helpers,
imports/exports, enum associations, visual schema branches and the parameter
reference. Package users run no generator.

Normal new effects default to `vestra.effects.stylize` with no Python metadata
entry. Optional `scripts/effects/python_metadata.py` overrides own module
placement, constructor ergonomics, naming and historical compatibility. Keep
special validation timing, inheritance and lowering adapters explicit. Existing
`PseudoAscii` and the Ascii schema/parser distinction remain compatibility cases.

Commit generated changes with the declaration. Do not hand-edit generated
classes or refresh frozen expectations to hide a changed contract.
When adding a public effect, update tests that deliberately pin the catalog's
entry counts, while retaining existing compatibility specimens and adding tests
for the new behavior.

## Verify the result

In `nix develop` or the equivalent environment, run:

```bash
just effect-check
uv sync --locked --extra dev --reinstall-package vestra
just python-test
just docs-check
just check
```

`effect-check` runs generation, core/common/CPU tests and WGPU library tests
excluding `gpu_` integration-test names, and checks included semantic-source
formatting. Two readback callback tests still request a device when an adapter
is available; use the pinned development environment for a usable adapter. Use `just fmt` to format. After Rust-only edits, the explicit
reinstall above ensures Python loads a newly built extension; ordinary uv
synchronization can reuse its cached native build. Run adapter checks separately:

```bash
just wgpu-software
just wgpu-hardware
```

Follow [GPU validation](gpu-validation.md) to discover/name the adapter.
Software tests do not establish hardware correctness. Inspect actual frames:
alpha, masks/groups, chains, boundaries, repeated and out-of-order requests.
Compare literal/reference pixels as well as CPU/WGPU parity. See
[testing](testing.md) for isolated features, frozen contracts, schema and
packaging, and [performance](performance.md) for comparable measurements.

Compile-time checks catch missing variants/modules/functions, operations and
backend decisions; duplicate kernel bits fail constant assertions. Whole-catalog
structural tests check scalar/plain-track visitors, compiled constraints and
negative keyframe-validation paths. Registered GPU records receive ABI checks.
Frozen specimens and actual pixels supply further compatibility evidence;
these are not exhaustive proofs of every parameter combination or device.

Add a specimen to the whole-catalog conformance table in
`crates/vestra-render-common/src/kernel.rs` for each public effect. A new kernel
also needs new rows in the frozen kernel-order, shader-byte and packed-record
tests; keep existing rows unchanged. Catalog-count tests intentionally require
updates too. These test edits are additional to the implementation footprint.

Common mistakes are identity rules ignoring animation/modifiers, omitted
conservative passes, wrong original/secondary inputs, incomplete validation,
wrong WGSL field order, and treating shader parsing as renderer validation.
Add effect-specific regression tests for those decisions.
