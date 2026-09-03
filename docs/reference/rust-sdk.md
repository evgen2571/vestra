# Rust SDK

The `vestra` crate is the public Rust SDK. Core model and renderer internals
are not part of this overview unless re-exported by the SDK.

## Main types

`vestra::Project` loads and saves canonical projects and can create immutable
snapshots. `vestra::Editor` is the operation entry point. It exposes
`validate`, `preflight`, `inspect`, `prepare`, and one-shot `render`.

`PrepareOptions` selects preparation behavior. `PreparedProject` owns reusable
prepared state and renders frames or video. `RenderRequest` is the one-shot
render request. Prepared video rendering uses the prepared-project request
type where it is exposed by the current API.

`RenderEvent`, `RenderResult`, `RenderTimings`, `RenderPerformance`,
`RenderFailureContext`, and `RenderFailureStage` describe progress, successful
publication, timings, and render failures. `CancellationToken` cancels a
cooperative render.

For normal rendering, use `Editor::render_auto(...)`, which applies the
default `ProgressMode::Auto` presentation. Use `render_with_progress(...)` when
supplying a `ProgressSink`, and `render_with_observer(...)` for an observer that
also needs cancellation control. The compatibility callback-oriented
`Editor::render(...)` remains available but is not the recommended default.

## Validation and errors

`ValidationReport` is canonical semantic validation. `PreflightReport` adds
resource, media, output, and backend readiness checks selected by
`PreflightOptions`. `EditorError` separates project, plan, and render failures
and exposes diagnostics, warnings, timings, cancellation status, and render
failure context.

## Backend types

`BackendPreference` requests `auto`, `cpu`, or `wgpu`. The result reports the
selected backend, any fallback, and adapter metadata when WGPU supplies it.
`BackendKind`, `GraphicsBackend`, and `AdapterDeviceType` describe different
things. WGPU selection is not a hardware-acceleration claim.

Use rustdoc for the complete signatures and trait bounds. This page records
the stable navigation-level contract, not private renderer modules.
