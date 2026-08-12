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
        -> vestra SDK
        -> vestra-cli
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

Scalar signals follow the same boundary. Canonical properties describe their
source and ordered transforms. The compiler interns equivalent complete signals
and derives raw analysis requirements. Media produces only raw Master RMS,
peak, and band-energy series, while preparation applies signal transforms once.
Frame evaluation samples the prepared series at absolute project time before it
applies property modifiers and target-domain constraints. Render backends receive
only evaluated numeric values and have no signal or audio-analysis semantics.
Empty requirements skip analysis entirely, even when ordinary output audio is
enabled.

The compiler also normalizes its private visual plan. `TemporalDependency::Static`
means a layer, timed effect, or post-effect chain has the same rendered content
at every time in its active domain; `Dynamic` means a track or active interval
can change that content. Transform contributions use half-open intervals and a
non-identity contribution is dynamic unless it covers its owner duration. A
layer's timeline activation is separate from that classification. Exact identity
effects are omitted before evaluation. A complete
normal-blend static basic-colour chain may become one internal `ColourTransform`;
renderers execute that compiled operation and do not rediscover identities or
fusion opportunities. This metadata is not project JSON. Phase 10B consumes it
by caching completed layer-local output before destination-dependent composition:
CPU retains immutable `Arc<RgbaImage>` values and WGPU retains sampled GPU
textures. Both caches are bounded by the prepared plan's existing cache budget.

Phase 10C keeps mutable workspace separate from that immutable cache. The CPU
backend owns three fixed RGBA8 full-canvas effect surfaces (`current`,
ping-pong target, and Gaussian horizontal scratch), bounded by its single-frame
capacity. They are reset only when a pass needs prior pixels cleared, reused for
layer-local and post effects, and never alias a cached `Arc<RgbaImage>` or a
completed frame. Cache publication transfers the finished scratch image into
the immutable cache and replaces that pool member. Final CPU frames still own a
new RGBA vector because the completion queue and `FrameSink` may retain it.

WGPU owns a fixed canvas/layer/effect texture set for the prepared project's
dimensions, `Rgba8Unorm` format, and working usage flags. The queue orders each
submitted command buffer, including its final texture-to-readback-buffer copy,
before the next command buffer writes the same transient textures; readback
slots separately retain per-submission buffers until map completion. Static
cache textures are never part of this mutable working set. Readback allocates
one final owned tight RGBA vector and copies each valid mapped row directly into
it; there is no second tight-packed intermediate. FFmpeg synchronously writes
that vector by slice without a renderer-side clone.

## Rendering

`render/geometry` owns crop materialization, image sizing, and resolved forward
and inverse affine transforms. It also materializes transformed image corners.
CPU rasterization uses those corners for visible bounds and the inverse mapping
for sampling. WGPU parameter packing uses the same source region, dimensions,
and inverse mapping, converting only final uniform values to `f32`.

`render/metrics` owns the flat preparation and timing report types. The engine
uses its merge methods for compiler, schedule, and backend data.

`crates/vestra-render/src/decoded.rs` eagerly decodes image assets once.
`crates/vestra-render/src/cpu/assets.rs` owns the CPU-only static-crop
cache. `vestra-core` plans backend-neutral logical effect passes,
including identity elimination, multipass decomposition, and pass order.
`vestra-render` executes those logical passes on CPU and WGPU. The WGPU executor
receives self-describing planned passes and never rediscovers effect semantics.

`crates/vestra-render/src/cpu` contains its prepared backend, composition, rasterization, surface
reuse, and CPU effect algorithms. The CPU backend is ready when its constructor
returns. For dynamic visuals, `CpuBackend` dispatches complete frames to a
persistent, automatically bounded set of workers. Each worker exclusively owns
one `CpuWorkerState` (prepared crop cache, static-layer cache, effect-surface
pool, and counters), while all workers share immutable decoded assets through
`Arc<DecodedAssets>`. Worker-local cache budgets are quotient/remainder
partitions of the configured class budget: all crop caches combined and all
static-layer caches combined are each bounded by `maximum_cache_bytes`.
Automatic worker selection reserves one logical CPU when possible, applies a
checked full-frame working-set estimate, and is capped at eight workers. The
engine remains responsible for bounded staging, cancellation, and ordered
`BTreeMap` delivery; CPU completion order is not encoder order.

`crates/vestra-render/src/wgpu/backend.rs` owns prepared WGPU state and staged frame submission. `context`
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

`crates/vestra-render` owns `CompletedFrame`.
`crates/vestra/src/application/render.rs` owns the SDK-private
preparation bridge; `crates/vestra/src/render/engine/runner.rs` owns its
private `PreparedState`. Preparation compiles one immutable shared plan, decodes
visual assets, compiles the schedule, selects and constructs the backend, and
uploads GPU resources once. It retains a stable preparation-timing snapshot and
preparation facts. A video operation separately owns its output check,
`FfmpegSink`, ordering buffer, operation metrics, and publication. Cumulative
backend counters are sampled at the operation boundary and rendered as deltas.
Static-cache hits, misses, budget bypasses, and layer renders are per-operation
deltas. Static-cache entries and estimated bytes are gauges after that
operation. WGPU reserves a static-cache entry before allocating its texture;
retained estimated bytes plus pending reservations never exceed the configured
static-cache budget. For CPU, `maximum_cache_bytes` independently bounds each
cache class across all worker-local instances: aggregate crop-cache capacity
and aggregate static-layer-cache capacity each remain at or below that value.
WGPU semantics are unchanged; its cache limit remains independent of other
internal caches and is not a total renderer-memory or physical-VRAM cap.

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
It reports preparation facts and can render repeated videos or CPU/WGPU frames
without rebuilding the plan, decoded assets, schedule, or backend.

The schedule itself is immutable; each video operation creates a chronological
cursor and active-layer list. Both the cursor path and arbitrary frame lookup
apply the same core `DrawKey` ordering helper, so insertion/event order cannot
change CPU compositing. Frame timestamps use checked rational integer math:
`Frame::timestamp()` is the earliest representable `Duration` that maps to the
frame (a ceiling at fractional boundaries), while the final duration remains
exclusive. Single-frame operations then follow the same staged submit,
completion, flush, and idle-verification contract. WGPU frame readback copies
its padded GPU rows into owned tightly packed RGBA8 storage before a slot can be
reused. `crates/vestra/src/render/engine/frame_loop.rs` owns cancellation, active-layer
updates, evaluation, backend rendering, completion ordering, delivery to
`FrameSink`, and progress events. The frame loop depends only on `FrameSink`. It
never knows about FFmpeg, process arguments, or temporary paths.

`vestra-media` owns `FrameSink`, `FfmpegSink`, encoding, and output
publication. A sink validates and writes the root's ordered input. `FfmpegSink`
closes stdin, terminates and reaps its child on abort or active drop, joins stderr
collection after process resolution, and retains the child handle when cleanup
fails so a later abort or `Drop` can retry. The runner publishes only after sink
finalization reports the expected frame count. Failure handling removes temporary
output. Cancellation remains the primary diagnostic, while a sink cleanup failure
is attached as a secondary hint.

For a compiler-proven static final visual, the prepared state retains one
immutable final RGBA template. Normal FFmpeg file output writes that template
once as a temporary PNG and gives FFmpeg a looping image input at index zero.
Audio inputs therefore remain indexed from one. FFmpeg receives the compiled
frame count and performs the repeated video-frame generation internally.

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

## Extending visual and audio effects

Effect catalogs and descriptors in `vestra-core` are the source for
serialized IDs, authored parameter metadata, generated schema, and Python
generic authoring. After changing an authored effect, update its compiler and
evaluator lowering and add catalog, descriptor, schema, and typed/generic
parity tests. Run the registered-effect conformance tests; they check that
runtime visual kernels remain covered by conservative compiled requirements.

A visual effect that lowers to existing `EffectOperation`/`EffectKernel`
primitives does not require renderer changes. A new visual primitive requires
the operation-to-kernel mapping, CPU implementation, WGPU capability and
pipeline/shader, parameter encoding, and backend conformance/parity tests.

An audio effect that lowers to an existing `AudioEffectOperation` needs no
`vestra-media` change: media consumes only the compiled operation plan.
A new audio primitive requires its duration behavior, core sample-domain
rounding and plan propagation, FFmpeg lowering, exact output-duration
enforcement where needed, and timeline/analysis parity tests.

A duration-changing audio primitive must also declare valid scope, define
backend-neutral duration transformation and rounding semantics, propagate
selected and processed duration through the plan, and prove encoder/analyzer
parity. It cannot be implemented as an FFmpeg filter alone. Finish changes by
running `cargo run -p vestra-cli -- generate-schema`, the schema
freshness/self-validation checks, the Rust workspace tests, and the focused
Python authoring tests.
# Audio timeline (schema v2)

Audio semantics live in `vestra-core`: `AudioTimeline` owns ordered
`AudioTrack` lanes, and each track owns ordered `AudioClip` placements. Clips
may overlap within or across tracks. The compiler produces a backend-neutral
`AudioMixPlan`; FFmpeg syntax stays in the media crate. The media executor
opens each unique audible resolved path once in first-use order, normalizes it
to 48 kHz stereo `fltp`, and uses `asplit` for repeated clip branches. Each
branch trims and places itself in the sample domain, then the executor mixes
clips into tracks, applies track gain, and mixes the tracks with
`amix=...:duration=longest:dropout_transition=0:normalize=0`.
Clip-local gain automation and audio fades run in the FFmpeg `aeval` stage,
which evaluates the envelope for every 48 kHz sample. The automation compiler
uses a balanced conditional dispatch tree, so expression nesting grows
logarithmically with the number of keyframes. FFmpeg's `volume` filter applies
static gain only because it evaluates a value once or per audio frame. Equal-power fades use
sine for fade-in and cosine for fade-out.
Audio gain automation uses source-keyframe interpolation: a keyframe controls
the segment from itself to the next keyframe. Visual animation tracks instead
assign interpolation to the segment ending at a keyframe. The final audio gain
keyframe has no following segment, so its interpolation is unused and its gain
holds through the selected clip end.
Seconds round to the nearest mixer sample, with ties upward for non-negative
schema times. The master is padded or trimmed to the resolved project length.
Large generated filtergraphs use a temporary file passed through FFmpeg 7+'s
`-/filter_complex` option once their UTF-8 text exceeds 64 KiB; smaller graphs
use `-filter_complex`. A render opens at most 128 deduplicated audible source
paths, checked before FFmpeg starts.
Mute, zero gain, and `output.audio` do not skip validation or change automatic
project duration.
