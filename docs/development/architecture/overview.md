# Architecture overview

Vestra's public Rust SDK, `vestra`, coordinates project meaning, frame rendering
and media I/O. Python's mutable authoring objects lower to a canonical snapshot;
the `vestra-python` PyO3 extension passes that snapshot and runtime requests to
the SDK. Rust applications and the `ve` CLI use the same SDK operations.

<figure class="diagram" markdown="span">
![Python authoring lowers a snapshot through the PyO3 extension to the vestra SDK. The SDK also serves Rust and the CLI, and coordinates core planning, CPU or WGPU rendering, and media I/O.](../../assets/diagrams/architecture-system.svg)
<figcaption>Arrows show calls and handoff through the SDK. This is an ownership view, not a Cargo dependency graph.</figcaption>
</figure>

## Keep each boundary clear

- **Core defines meaning.** `vestra-core` owns the canonical model, semantic
  validation, timeline compilation and frame-time evaluation. It does not open
  files, invoke FFmpeg or hold GPU state.
- **Renderers execute evaluated work.** `vestra-render` selects CPU or WGPU.
  Both backends implement the same project semantics; shared rendering contracts
  and prepared assets live in `vestra-render-common`.
- **Media owns bytes and files.** `vestra-media` probes and decodes assets,
  executes audio work, encodes frames and publishes output. It has no renderer
  dependency.
- **The SDK joins the work.** `Editor` coordinates validation, preflight,
  preparation and execution. It bridges owned renderer frames to borrowed media
  writes and reports success only after output publication.

Semantic validation checks project data without reading assets. SDK preflight
resolves paths and probes media, so environment failures can be reported before
frame execution. Preparation then creates reusable runtime state.

## Follow an operation

| Stage | Owner and handoff | Failure to look for |
| --- | --- | --- |
| Author and snapshot | Python lowers mutable objects; Rust/CLI load canonical data. | Invalid authoring values, JSON parsing or unsupported fields. |
| Validate and compile | Core checks semantics and builds the plan using SDK-supplied media facts. | IDs, scope, timing, resource limits or unresolved duration. |
| Preflight and prepare | SDK coordinates media probes, shared assets and backend preparation. | Missing/corrupt media, unusable output, unavailable adapter or device resources. |
| Evaluate and render | Core evaluates frame time; CPU/WGPU consume the evaluated plan. | Frame-time values, pass/resource constraints, decode or renderer failure. |
| Encode and publish | Media writes/finalizes; SDK coordinates cancellation and completion. | Encoder write, finalization or publication failure. |

These stages have different success boundaries: pure validation does not prove
media readiness, a WGPU selection does not prove hardware execution, and the
last frame does not prove publication. The [render pipeline](render-pipeline.md)
shows the operation order and retained state; the table identifies owners rather
than a strict sequence of calls.

Use [crate ownership](crates.md) to choose where a change belongs,
[project to plan](project-to-plan.md) for compilation and timing contracts, or
[render pipeline](render-pipeline.md) to follow a prepared project through execution.
