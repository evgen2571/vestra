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
        -> video-editor SDK
        -> video-editor-cli
```

Project code never depends on rendering. The plan does not depend on a backend.
The SDK coordinates loading, environment preflight, plan compilation, backend
selection, staged rendering, progress, and cancellation. The CLI only
translates arguments, installs Ctrl-C handling, and formats structured SDK
results.

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

`crates/video-editor-render/src/decoded.rs` eagerly decodes image assets once.
`crates/video-editor-render/src/cpu/assets.rs` owns the CPU-only static-crop
cache. `video-editor-core` plans backend-neutral logical effect passes,
including identity elimination, multipass decomposition, and pass order.
`video-editor-render` executes those logical passes on CPU and WGPU. The WGPU executor
receives self-describing planned passes and never rediscovers effect semantics.

`crates/video-editor-render/src/cpu` contains its prepared backend, composition, rasterization, surface
reuse, and CPU effect algorithms. The CPU backend is ready when its constructor
returns.

`crates/video-editor-render/src/wgpu/backend.rs` owns prepared WGPU state and staged frame submission. `context`
creates the adapter and device. `requirements` validates limits and estimates
allocation sizes before resources exist. `frame_plan` creates and validates
adapter-independent canvas and effect ping-pong operations. `texture_pool` owns
Canvas A, Canvas B, Layer, Effect A, and Effect B for the backend lifetime.
`pipeline` creates texture compute
pipelines and layouts. `parameters` stores aligned per-operation records.
`executor` reuses per-slot bind groups, encodes one complete frame into one
command buffer, and submits it once. `readback` owns the bounded slot ring,
generation tokens, asynchronous callbacks, mapping, unmapping, and row
repacking. `polling` is the only module that calls `device.poll`. Support checks
decide whether the current WGPU renderer accepts a plan. That policy is outside
the engine so future GPU effect work changes the WGPU module rather than
selection logic.

`RenderBackend` accepts evaluated frames only. `submit_frame` starts work without
returning pixels. `poll_completed` advances callbacks in nonblocking or waiting
modes, and `flush` drains every submitted frame. The engine creates CPU and WGPU
backends lazily and returns a fully prepared backend before frame rendering
starts. CPU preference never initializes WGPU. Auto selection tries WGPU first
and constructs CPU only when WGPU preparation fails. A failure after frame
rendering begins aborts the render instead of changing backends mid-stream.

## Engine, failures, and output

`crates/video-editor-render` owns `CompletedFrame`.
`crates/video-editor/src/application/render.rs` owns the SDK-private
preparation bridge; `crates/video-editor/src/render/engine/runner.rs` owns its
private `PreparedState`. Preparation compiles one immutable shared plan, decodes
visual assets, compiles the schedule, selects and constructs the backend, and
uploads GPU resources once. It retains a stable preparation-timing snapshot and
preparation facts. A video operation separately owns its output check,
`FfmpegSink`, ordering buffer, operation metrics, and publication. Cumulative
backend counters are sampled at the operation boundary and rendered as deltas;
cache request fields are per-operation deltas while cache occupancy fields are
the persistent snapshot after that operation.

After the staged frame loop succeeds, the runner verifies backend idleness
before encoder finalization and publication. CPU requires an empty completion
queue; WGPU requires no in-flight or unavailable readback slot and no aborted
state. An idle-invariant failure aborts and invalidates the prepared state,
cleans the temporary output, and emits no completed event. Any failure after
frame submission conservatively invalidates the state; an invalidated state is
never rebuilt implicitly. Output, FFmpeg, and pre-submission cancellation
failures leave it reusable. The visual snapshot is owned by the prepared state,
while operation-time audio remains an FFmpeg input and must remain available and
unchanged for deterministic repeated video renders. The SDK exposes this
ownership through `Editor::prepare`, which returns an opaque `PreparedProject`.
It reports preparation facts and can render repeated videos or CPU frames
without rebuilding the plan, decoded assets, schedule, or backend.

The schedule itself is immutable; each video operation creates a chronological
cursor and active-layer list. Both the cursor path and arbitrary frame lookup
apply the same core `DrawKey` ordering helper, so insertion/event order cannot
change CPU compositing. Frame timestamps use checked rational integer math:
`Frame::timestamp()` is the earliest representable `Duration` that maps to the
frame (a ceiling at fractional boundaries), while the final duration remains
exclusive. Single-frame operations then follow the same staged submit,
completion, flush, and idle-verification contract. WGPU frame readback remains
explicitly unsupported until Phase 6C. `crates/video-editor/src/render/engine/frame_loop.rs` owns cancellation, active-layer
updates, evaluation, backend rendering, completion ordering, delivery to
`FrameSink`, and progress events. The frame loop depends only on `FrameSink`. It
never knows about FFmpeg, process arguments, or temporary paths.

`video-editor-media` owns `FrameSink`, `FfmpegSink`, encoding, and output
publication. A sink validates and writes the root's ordered input. `FfmpegSink`
closes stdin, terminates and reaps its child on abort or active drop, joins stderr
collection after process resolution, and retains the child handle when cleanup
fails so a later abort or `Drop` can retry. The runner publishes only after sink
finalization reports the expected frame count. Failure handling removes temporary
output. Cancellation remains the primary diagnostic, while a sink cleanup failure
is attached as a secondary hint.

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
compiler, compiled-effect metadata, evaluation, logical-pass planning in
`video-editor-core::plan`, CPU execution in `crates/video-editor-render/src/cpu`,
WGPU execution in `crates/video-editor-render/src/wgpu`, parameter encoding,
shader and pipeline mapping, prepared bind groups, capability coverage, a parity
fixture, and a benchmark. Keep the effect-plan and evaluation behavior unchanged
so neither renderer rediscovers ordering.
