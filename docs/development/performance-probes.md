# Performance probes

Use these opt-in investigations after a matched [baseline comparison](performance.md)
identifies the stage or resource that changed. Run from the repository root in
the native contributor environment. Use release builds, fresh output paths and
serial runs; capture profiling separately from uninstrumented timing.

These probes have different timing scopes and report formats. Compare matched
runs of one probe, not its timing with a one-shot encoded suite. Record the
[reproducibility details](performance.md#measurement-scope) with every result.

## Isolate preparation, effects or random access

Run the gated SDK tests for the stage you need:

```bash
VESTRA_RENDER_BENCH=1 cargo test --release -p vestra --no-default-features --features cpu preparation_matrix -- --nocapture
VESTRA_RENDER_BENCH=1 cargo test --release -p vestra --no-default-features --features cpu effect_scaling_matrix -- --nocapture
VESTRA_RENDER_BENCH=1 cargo test --release -p vestra --no-default-features --features cpu random_access_matrix -- --nocapture
```

Outputs are `target/benchmark-results/render-preparation.json`,
`render-effect-scaling.json` and `render-random-access.json`, respectively.
Outputs use the probes’ prepared/null-sink format rather than the end-to-end
suite schema.

## Local-canvas allocation probe

Discover an adapter with [GPU validation](gpu-validation.md) first. Replace
`vulkan` below with that API; `VESTRA_REQUIRE_WGPU=1` makes unavailable WGPU
fail rather than silently skip the probe. For hardware measurements, establish
an integrated/discrete classification through the validation procedure and retain
the actual probe adapter output. The availability flag alone does not establish
hardware execution. Run one case per process:

```bash
mkdir -p target/benchmark-results/group-canvases
VESTRA_WGPU_BACKEND=vulkan VESTRA_REQUIRE_WGPU=1 \
VESTRA_GROUP_BENCH_CASE=mixed \
VESTRA_GROUP_BENCH_REPORT=target/benchmark-results/group-canvases/mixed.json \
cargo test --release -p vestra-render-wgpu --lib --features adapter-tests \
  gpu_sized_group_allocation_benchmark -- --ignored --nocapture --test-threads=1
```

Cases are `legacy`, `card`, `wide`, `mixed` and `nested`, at 1280×720. One warmup
preparation precedes five samples. Reports separate backend preparation,
sequential frame latency including readback, and 240-frame staged throughput at
depth three. Frames are evaluated before measurement. The probe checks expected
pixels and stable working-texture allocation counts across both frame loops.
Working bytes describe created texture payloads; measure resident/process GPU
memory externally. Run timing comparisons separately from memory profiling and
alternate frozen before/after executables to reduce order and clock effects.

## Allocation and sustained-render profiling

Use the existing opt-in `VESTRA_CPU_PROFILE` hot-path timings and an external
allocation/CPU profiler to locate work, rather than inferring it from cache hits.
Record the profiling tool, version, invocation and overhead separately from
uninstrumented paired measurements. Engine allocation/copy counters cover named
operations, not every allocator call. Distinguish allocation churn from retained
capacity and peaks; fewer allocations can retain more memory.

For process-memory measurements, include the renderer and owned FFmpeg child
processes, name the platform metric (for example RSS/high-water RSS), sampling
interval and collection method, and state whether peaks are simultaneous or
per-process maxima. GPU working-set counters are estimates, not device-resident
memory measurements. The canonical `just benchmark` runner records engine
counters; process-memory captures for that runner need external tooling. The
[portable collector](../benchmarks/collecting.md) also records the sampled
process-tree metrics described below. Missing measurements are unavailable,
not zero.

Inspect the [stage timings](performance.md#measurement-scope) alongside process
samples to distinguish renderer work from decode, encoding and publication.

For sustained rendering, sample early, steady and late intervals and repeated
prepare/render/abort cycles. Track throughput alongside process memory, file
handles, decoder contexts, worker scratch, cache reservations and in-flight
buffers. Streamed master PCM avoids retaining the whole waveform, but prepared
signal/analysis data can still scale with duration; growth is not automatically
a leak. Choose workloads within the checked arithmetic and resource budgets
described in [validation layers](../reference/project-format.md#validation-layers).
Report actual duration/frame count and output bytes: short smoke runs prove
execution, not multi-hour stability or large-file behavior. Changes involving
encoded output also need [media verification](media-pipeline.md#verify-encoded-output).

### Collector resource sampling

The [portable collector](../benchmarks/collecting.md) uses psutil to sample
simultaneous process-tree RSS, including observed FFmpeg descendants. Interpret
its resource records with these limits:

| Metric | Scope and limit |
| --- | --- |
| Process-tree RSS | Shared pages may be counted more than once; sampled peaks are lower bounds. RSS is not GPU VRAM. |
| CPU utilization | Observed-tree estimate with one core at 100%; short-lived children may be missed. |
| Missing measurements | Stored as null, never zero. |
| Sustained time series | Five-second samples (100 milliseconds in smoke), progress where exposed, output size and file-handle counts. |
| Decoder/cache counters | Final Vestra totals; live counters are unavailable. |

Compare early, steady and late throughput alongside RSS, buffering, allocator
retention and cache warmup. Memory growth alone does not prove a leak. The
collector's sustained workload avoids duration-dependent audio analysis; it
does not establish constant memory for arbitrary audio-heavy projects. A
three-second smoke proves neither multi-hour stability nor files larger than
4 GiB.

## Cache working-set and native worker diagnostics

The ignored SDK `static_cache_threshold_benchmark` reuses the existing mixed-layer
fixtures and null-sink runner. Run it in release mode with a fresh output path:

```bash
mkdir -p target/benchmark-results/cache-diagnostic
VESTRA_CACHE_BENCH_OUTPUT=target/benchmark-results/cache-diagnostic/result.json \
VESTRA_CACHE_BENCH_WORKLOAD=mixed-layers-10 VESTRA_CACHE_BENCH_MIB=256 \
VESTRA_CACHE_BENCH_MODE=timing cargo test --release --locked -p vestra \
  --no-default-features --features cpu static_cache_threshold_benchmark \
  -- --ignored --nocapture --test-threads=1
```

Compare 256 and 320 MiB in repeated blocks. Timing mode emits one warmup and
five independently prepared sequences, each with cold/warm/warm-2 operations.
The operations within one sequence reuse prepared state. `partitions` mode
separately captures actual per-worker budgets; `pixels` compares every RGBA byte
through the SDK frame loop with a cache-disabled reference. Keep both diagnostic
modes outside performance/resource controls. Supported controls are mixed8,
mixed10, mixed25, mixed50 (`mixed-layers-N`) and `static-image`, all at 720p/30
frames. Dynamic controls require the same eight automatic workers; the static
control uses one. These are test-only budget controls.

### Collector cache and decoder methodology

Use the [collection guide](../benchmarks/collecting.md#optional-profiles) to run
the optional cache profile. Its two-hour source repeats a detailed moving
12-second H264 segment by stream copying: storage is roughly 600 times the
segment size, without a two-hour encode. Preparation decodes all presentation
timestamps, checks stream-copy boundaries and compares early/middle/late seek
pixels. This takes time even when no endurance output is requested.

The access matrix uses a single native worker and the existing isolated
decoded-RGBA cache option, with zero versus eight-frame budgets per decoder.
Static, bitmap and crop caches are unchanged and are not attributed to this
option. A separate correctness pass compares exact bytes against a
cache-disabled reference before timing.

| Request pattern | What it exercises |
| --- | --- |
| Sequential access | Decoder cursor reuse with little need for LRU hits |
| Same-frame holds | Current-frame retention, which also works with zero LRU budget |
| Local backward reuse | Reuse of decoded frames within the cache budget |
| Seeded random access | Fixed requests across the long source |
| Budget overflow and revisits | Explicit working sets larger than the cache; native eviction counts are unavailable |
| Two decoder sessions at different offsets | Isolated session reuse for repeated placements |

A separate three-second public-API encoded render places that same source
concurrently at offsets zero and six seconds. It exercises the CPU worker-pool
placement policy; keep its wall time separate from the isolated decoder-access
A/B measurements. The sustained profile also uses two moving placements, with
opacity, brightness and animated positioning, at 640×360 / 30 fps. Cache and
sustained smoke share the same 128×72 two-hour source to bound decoding cost.

Forward jumps in the native decoder decode intervening frames; backward
requests seek. Random long-source access can therefore be expensive enough to
hit a worker timeout. Low sequential cache-hit counts are expected. The
collector measures this behavior without changing the engine.

MoviePy public `get_frame` latency and process memory are separate observations;
there are no equivalent native cache/decode counters to report. Initial
`VideoFileClip` opens are reported separately; internal decoder/process reopen
counts are unavailable. Keep three conclusions separate: native cache
correctness, native A/B benefit for a particular request pattern, and MoviePy
end-to-end access latency.

### Native worker sweep

The CPU crate also has ignored `native_workers_1`, `native_workers_2`,
`native_workers_4` and `native_workers_8` tests. These use a test-only native
FFmpeg owner-thread adapter and the existing private fixed-worker constructor.
They expose no production worker-count setting. Supply
`VESTRA_NATIVE_BENCH_INPUTS`
with five video-only, source-sized scene JSON files (`single-video`, `video-heavy`,
`holds`, `trimmed`, `divergent-clocks`) and `manifest.json`. The manifest maps asset
IDs to source paths, full external packed-RGBA reference files, integer PTS lists,
time-base numerator/denominator and source origin. Generate references using full
external FFmpeg decoding with bilinear conversion and passthrough frame timing;
use ffprobe's actual presentation timestamps rather than nominal FPS.

Select a fresh `VESTRA_NATIVE_BENCH_OUTPUT`, `VESTRA_NATIVE_BENCH_CASE`,
`VESTRA_NATIVE_BENCH_SINK=null|encoded` and `VESTRA_NATIVE_BENCH_MODE=oracle|timing`,
then run the chosen ignored test in release mode, for example:

```bash
# Set the five VESTRA_NATIVE_BENCH_* variables described above first.
cargo test --release --locked -p vestra-render-cpu native_workers_1 \
  -- --ignored --nocapture --test-threads=1
```

These fixtures are 1280×720, 30/1 and one second. Oracle mode checks pixels/PTS
and captures per-session requests, receives, seeks, conversions and waits;
timing mode disables those
additional diagnostics and emits one warmup plus five fresh-backend samples.
Run null controls before matched encoded controls, repeat the worker sweep, and
record process resources separately. Worker count changes cursor count and
per-worker cache partitions together. These internal controls do not measure
public SDK end-to-end throughput or justify a new scheduling policy by themselves.
