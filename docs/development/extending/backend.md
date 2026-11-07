# Add a backend

A backend renders Vestra's prepared/evaluated plan. It implements the common
[`RenderBackend` lifecycle](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-common/src/backend.rs),
accepts shared prepared resources and returns owned RGBA frames. Start with the
[CPU implementation](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-cpu/src/cpu/backend.rs)
and compare [WGPU's implementation](https://github.com/evgen2571/vestra/blob/develop/crates/vestra-render-wgpu/src/wgpu/backend.rs)
when submissions can remain in flight.

## Find the owners

| Responsibility | Location |
| --- | --- |
| Lifecycle, completed frames and shared assets | `crates/vestra-render-common/src/` |
| Execution, capability policy and private resources | New implementation crate, following CPU/WGPU ownership |
| Backend selection, availability and fallback | `crates/vestra-render/src/lib.rs` and facade features |
| Preparation, frame scheduling and reports | `crates/vestra/src/` |
| Frame sink and encoded-output lifetime | `crates/vestra-media/src/`; no backend dependency |

## Implement in order

1. Implement the lifecycle and capability validation against common kernel
   mapping/requirements. Shared immutable image/shape/text/glyph preparation
   stays in common. The backend must not open project paths or repeat semantic
   validation.
2. Prepare private surfaces or device resources, then render evaluated sources,
   effects and groups. Preserve source adaptation, composition-local timing,
   layer/effect order, masks/mattes, alpha and public frame ownership. Report
   metrics and errors. An asynchronous implementation also needs bounded
   in-flight work and completion-safe resource retirement.
3. Wire selection and fallback through the facade, which the SDK calls during
   preflight/preparation. Keep requested preference, selected backend and
   fallback distinct in `PreparationReport` and `RenderResult`. Add truthful
   adapter/device information only where it has an analogue.
4. Map failures to preparation/render diagnostics with stage context. SDK
   orchestration keeps progress events, cancellation checks and the bridge from
   owned frames to media's borrowed RGBA writes. Do not add a renderer dependency
   to media.

## Verify and document

Test lifecycle/capacity, retained output ownership, cancellation, resource reuse,
fallback and unavailable-backend errors. Render canonical scenes covering each
claimed source/effect, nested compositions and encoded output. Add facade feature
isolation checks and update public selection/reference documentation; use
[Testing](../testing.md#backend-feature-isolation) and
[media verification](../media-pipeline.md#verify-encoded-output).

The change is ready when demonstrated semantics and the support matrix agree.
State missing coverage as `unsupported`, `partially supported` or
`not fully verified` as appropriate. Compilation alone does not establish parity;
use [GPU validation](../gpu-validation.md#understand-the-test-evidence) for
adapter evidence and rendered-output inspection.
