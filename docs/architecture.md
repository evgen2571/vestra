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
asset-table construction, one visible clip's source and track compilation,
compiled-plan limits, effects, presets, transitions, flashes, audio, output
settings, time conversion, and workload metrics. `plan/evaluation` evaluates
the plan for one frame. It owns transform contributions, effects, colour
transforms, camera shake, and motion calculations.

## Rendering

`render/geometry` owns crop materialization, image sizing, and resolved forward
and inverse affine transforms. It also materializes transformed image corners.
CPU rasterization uses those corners for visible bounds and the inverse mapping
for sampling. WGPU parameter packing uses the same source region, dimensions,
and inverse mapping, converting only final uniform values to `f32`.

`render/metrics` owns the flat preparation and timing report types. The engine
uses its merge methods for compiler, schedule, and backend data.

`render/decoded` eagerly decodes image assets once. `render/cpu/assets` owns the
CPU-only static-crop cache. `render/effects` turns evaluated effects into
backend-neutral logical passes. It owns identity elimination, multipass
decomposition, and pass order. The CPU effect executor consumes those passes
with its established surface pool and algorithms. A future WGPU executor can
consume the same logical pass plan.

`render/cpu` contains its prepared backend, composition, rasterization, surface
reuse, and CPU effect algorithms. The CPU backend is ready when its constructor
returns.

`render/wgpu/backend` owns prepared WGPU state and frame rendering. `context`
creates the adapter and device. `requirements` validates limits and estimates
allocation sizes before resources exist. `frame_plan` creates and validates
adapter-independent canvas ping-pong operations. `texture_pool` owns Canvas A,
Canvas B, and Layer for the backend lifetime. It only allocates effect textures
when executable effect support requests them. `pipeline` creates texture compute
pipelines and layouts. `parameters` stores aligned per-operation records.
`executor` reuses prepared bind groups, encodes one complete frame into one
command buffer, and submits it once. `readback` keeps synchronous mapping and
row repacking. Support checks decide whether the current WGPU renderer accepts
a plan. That policy is outside the engine so future GPU effect work changes the
WGPU module rather than selection logic.

`RenderBackend` accepts evaluated frames only. The engine creates CPU and WGPU
backends lazily and returns a fully prepared backend before frame rendering
starts. CPU preference never initializes WGPU. Auto selection tries WGPU first
and constructs CPU only when WGPU preparation fails. A failure after frame
rendering begins aborts the render instead of changing backends mid-stream.

## Engine, failures, and output

`render/engine/runner` coordinates output setup, shared decoding, active
scheduling, backend selection, FFmpeg startup, finalization, publication, and
final reporting. `engine/frame_loop` owns cancellation, active-layer updates,
evaluation, backend rendering, encoder writes, and progress events. Companion
modules hold public types, selection, events, and failure handling.

Failure handling cleans temporary output and aborts FFmpeg when needed. It keeps
completed-frame and attempted-frame accounting separate so progress and failure
reports preserve their existing meaning.

## Effect and geometry flow

```text
project effect
  -> validation
  -> compiled effect
  -> evaluated effect
  -> backend-neutral logical passes
  -> CPU pass executor
```

```text
evaluated source and transform
  -> shared crop and sizing resolution
  -> shared forward/inverse affine geometry
  -> CPU raster bounds and sampling
  -> WGPU parameter packing
```

## Extending effects

To add a project effect, update the project model and schema, validation,
compiler, compiled-effect metadata, evaluation, `render/effects` pass planning,
CPU execution, and compatibility tests. If WGPU does not implement it, update
`render/wgpu/support.rs` so explicit WGPU renders fail and automatic selection
falls back before the first frame.

To implement WGPU support later, keep the effect's plan and evaluation behavior
unchanged. Add the shared `EffectPass` to a self-describing `GpuOperation`, then
add typed GPU parameters, a shader, a pipeline, a prepared resource bind group,
an executor branch, a support declaration, a parity test, and a benchmark.
This keeps GPU capability policy in one place and avoids making the engine
rediscover effect semantics from an evaluated layer.
