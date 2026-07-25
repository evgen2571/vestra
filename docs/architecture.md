# Architecture

The editor keeps project syntax, semantic rules, planning, frame evaluation,
rendering, encoding, and command-line presentation in separate layers.

```text
JSON
  -> project model
  -> semantic validation
  -> validated project
  -> plan compiler
  -> render plan
  -> per-frame evaluation
  -> evaluated frame
  -> selected backend
  -> RGBA frame
  -> FFmpeg
  -> temporary output
  -> published output
```

## Dependency direction

Dependencies point down this sequence:

```text
domain / timeline / animation
        -> project model
        -> project validation
        -> plan model and compiler
        -> plan evaluation
        -> render geometry and backend-neutral support
        -> CPU or WGPU backend
        -> render engine / media / output
        -> application
        -> CLI
```

Project code never depends on rendering. The plan does not depend on a backend.
The CLI only translates arguments and reports into application calls.

## Project and plan

`project/model` owns the serde model and project-format types. `project/validation`
checks semantic rules and produces a validated project. Effect parameter validation
has one owner, while clip-local and post-effect scope rules remain separate.

`plan/compiler` turns a validated project into a `RenderPlan`. Its modules own
assets, tracks, effects, presets, transitions, flashes, audio, output settings,
time conversion, and workload metrics. `plan/evaluation` evaluates the plan for
one frame. It owns transform contributions, effects, colour transforms, camera
shake, and motion calculations.

## Rendering

`render/geometry` owns pure crop bounds, image sizing, and inverse-affine
calculations. CPU rasterization and WGPU parameter packing use those calculations
instead of maintaining separate formulas.

`render/metrics` owns the flat preparation and timing report types. The engine
uses its merge methods for compiler, schedule, and backend data.

`render/decoded` eagerly decodes image assets once. `render/cpu/assets` owns the
CPU-only static-crop cache. `render/cpu` contains composition, rasterization,
surface reuse, and CPU effect algorithms. The CPU backend is ready when its
constructor returns.

`render/wgpu` owns the WGPU implementation. `context` creates the adapter and
device. `requirements` validates limits before resources exist. `pipeline`
creates the shader and bindings. `resources` owns source textures and persistent
frame buffers. `executor` keeps the current per-layer submission behavior, and
`readback` keeps synchronous mapping and row repacking. Parameters pack shared
geometry for the shader. Support checks decide whether the current one-pass WGPU
renderer accepts a plan. That policy is outside the engine so future GPU effect
work changes the WGPU module rather than selection logic.

`RenderBackend` accepts evaluated frames only. The engine selects a fully
prepared backend before starting frame rendering. Auto selection may fall back
to CPU during preparation, but a failure after frame rendering begins aborts the
render instead of changing backends mid-stream.

## Engine, failures, and output

`render/engine` coordinates output setup, shared decoding, active scheduling,
backend selection, FFmpeg startup, finalization, publication, and final reporting.
`engine/frame_loop` owns cancellation, active-layer updates, evaluation, backend
rendering, encoder writes, and progress events. Companion modules hold public
types, selection, events, and failure handling.

Failure handling cleans temporary output and aborts FFmpeg when needed. It keeps
completed-frame and attempted-frame accounting separate so progress and failure
reports preserve their existing meaning.

## Extending effects

To add a project effect, update the model, shared parameter validator, compiler,
evaluator, CPU pass planning or algorithm, and compatibility tests. If WGPU
does not implement it, update `render/wgpu/support.rs` so explicit WGPU renders
fail and automatic selection falls back before the first frame.

To implement WGPU support later, keep the effect's plan and evaluation behavior
unchanged, add its shader and parameter or resource needs under `render/wgpu`,
then adjust only the support checker and parity tests. This keeps GPU capability
policy in one place and avoids making the engine understand individual effects.
