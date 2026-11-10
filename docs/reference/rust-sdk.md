# Rust SDK

The `vestra` crate is the public Rust SDK. Core model and renderer internals
are not part of this overview unless re-exported by the SDK.

## Main types

`vestra::Project` loads and saves canonical projects and can create immutable
snapshots. `vestra::Editor` is the operation entry point. It exposes
`validate`, `preflight`, `inspect`, `prepare`, and one-shot `render`.

`PrepareOptions` selects preparation behavior. `PreparedProject` owns reusable
prepared state and renders frames or video. `RenderRequest` is the one-shot
render request. Use `PreparedVideoRenderRequest` to render video from prepared state.

`RenderEvent`, `RenderResult`, `RenderTimings`, `RenderPerformance`,
`RenderFailureContext`, and `RenderFailureStage` describe progress, successful
publication, timings, and render failures. `CancellationToken` cancels a
cooperative render.

## Rendering

### Output overrides and animation formats

`RenderRequest` carries optional `resolution`, `frame_rate: FrameRate` and
`quality: Quality` overrides. `preview: true` defaults to a 640 px longest side,
at most 15 FPS and `Quality::Preview`; explicit fields take precedence.
`PrepareOptions::with_resolution`, `with_frame_rate` and `with_quality` fix the
same runtime settings for reusable preparation. They do not edit `Project`.
A different aspect ratio fits centrally without stretching the composition.

`PreparedVideoRenderRequest` and one-shot rendering select GIF and animated
WebP from the destination extension, using the existing frame sink and atomic
publication lifecycle. Both formats omit audio; visual audio analysis remains
available. See the [format and timing contracts](render-outputs.md#animation-formats)
for palette, transparency, timing quantization and container limits.

### Progress and cancellation entry points

For normal rendering, use `Editor::render_auto(...)`, which applies the
default `ProgressMode::Auto` presentation. Use `render_with_progress(...)` when
supplying a `ProgressSink`, and `render_with_observer(...)` for an observer that
also needs cancellation control. `Editor::render(...)` is the callback-oriented
entry point.

### Selected video ranges

Both `RenderRequest` and `PreparedVideoRenderRequest` support
`.with_range(start: Duration, end: Option<Duration>)`. The default starts at zero
and uses the project duration. `PreparedProject::frame_range(start, end)` returns
the corresponding half-open frame-index range without rendering.

Bounds round up on the rational project frame grid. Evaluation keeps absolute
project time, encoded timestamps start at zero, and audio is trimmed after
mixing and effects. See [range timing and errors](render-outputs.md#ranges-and-frame-boundaries).

## Authoring visual effects through JSON values

Visual effects use the canonical JSON/value route. With a mutable
`serde_json::Value` project document, add an ordered layer `effects` array or
`visual.post_effects` array, then load, prepare, and render through the SDK.
For example, this applies palette coloring followed by loopable fine dithering:

```rust
use serde_json::json;
use std::time::Duration;
use vestra::{BackendPreference, Editor, PrepareOptions, Project};

document["visual"]["post_effects"] = json!([
    {"id": "palette", "type": "palette_map",
     "palette": ["#001122", "#ffeecc"], "mode": "gradient",
     "amount": {"base_value": 1}, "phase": {"base_value": 0}},
    {"id": "dither", "type": "ordered_dither",
     "palette": ["#001122", "#ffeecc"], "mode": "nearest",
     "amount": {"base_value": 1}, "phase": {"base_value": 0},
     "period": 2, "strength": {"base_value": 1}, "matrix": "bayer8", "scale": 1}
]);
let project = Project::from_value(document, ".")?;
let mut prepared = Editor::new().prepare(
    &project, PrepareOptions::new(BackendPreference::Cpu),
)?;
let frame = prepared.render_frame(Duration::from_millis(500))?;
```

The calling crate needs `serde_json` for the value and `json!` macro. Palette
colors remain in authored dark-to-light order. Periods use owner-local seconds;
source media, keyframes, and audio must separately repeat for a whole scene to
loop. See the [effect reference](effects.md) for the canonical field contracts.

## Validation and errors

`ValidationReport` is canonical semantic validation. `PreflightReport` adds
resource, media, output, and backend readiness checks selected by
`PreflightOptions`. `EditorError` separates project, plan, and render failures
and exposes diagnostics, warnings, timings, cancellation status, and render
failure context.

`Editor::new()` and `Editor::default()` use safe resource limits. Configure the
same budgets across validation, preflight, preparation and rendering through
the builder:

```rust
use vestra::{Editor, ResourceLimits};

let editor = Editor::builder()
    .resource_limits(ResourceLimits {
        maximum_group_width: 16_384,
        maximum_group_pixels: 32_768,
        ..ResourceLimits::default()
    })
    .build()?;
```

Group budgets are independent of output dimensions. Defaults allow 8192 pixels
per axis and 67,108,864 rounded-up raster pixels in one local canvas. Larger
canvases require explicit budgets and a backend that supports them. Checked
dimension and RGBA byte-length representability always apply. See
[validation layers](project-format.md#validation-layers) for the diagnostic
contract and other resource budgets.

## Backend types

`BackendPreference` requests `auto`, `cpu`, or `wgpu`. The result reports the
selected backend, any fallback, and adapter metadata when WGPU supplies it.
`BackendKind`, `GraphicsBackend`, and `AdapterDeviceType` describe different
things. WGPU selection is not a hardware-acceleration claim.

Use rustdoc for the complete signatures and trait bounds. This page records
the stable navigation-level contract, not private renderer modules.
