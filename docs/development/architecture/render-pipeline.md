# Render pipeline

`Editor` is the runtime coordinator. `Editor::prepare` and one-shot `Editor::render` both start with semantic validation, then choose a preflight target. Preparation checks the requested backend and resources. A one-shot render also checks the destination and overwrite policy. If preflight has errors, `Editor` returns its diagnostics before resource preparation begins.

```text
ProjectSnapshot / Project
  -> validate
  -> preflight for preparation or render
  -> compile plan, decode assets, analyze audio, initialize renderer
  -> PreparedProject
  -> frame loop + audio graph + frame sink
  -> encoder finalization
  -> output publication
  -> RenderResult
```

`PreparedProject` owns the coordinated prepared state and its stable `PreparationReport`: compiled plan, decoded/prepared resources, selected renderer, adapter data, warnings and preparation timings. It can render individual frames or accept a `PreparedVideoRenderRequest` without repeating preparation. It is deliberately a runtime object rather than a serializable project. The Python wrapper protects one prepared state with a mutex, so concurrent use reports `PreparedProjectBusyError` instead of racing its backend resources.

The frame loop evaluates plan work at each timeline frame, submits it to the selected backend, writes owned RGBA output to the media sink, and emits `RenderEvent` values for callers. `CancellationToken` is cooperative. The loop checks it across the operation and a cancellation observed before publication cannot yield a successful result.

Within each layer, presentation ordering is source or child precomposition,
final transform, effects, ordered mask coverage, final opacity, and blend or
composite. Group masks therefore apply to the completed group result, while
child masks remain part of their child layers.

Media work has three distinct terminal boundaries: writing frames to the encoder, finalizing its temporary output, and publishing that output to the requested path. `RenderFailureContext` identifies the failed stage and may retain both temporary and destination paths. `RenderResult` and reports record requested and selected backend, fallback, adapter metadata, timings and performance counters. Treat the reported backend as the actual result, not the requested preference.
