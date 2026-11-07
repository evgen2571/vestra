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

Discover an adapter with [GPU validation](gpu-validation.md) first. Set
`VESTRA_WGPU_BACKEND` to that API and `VESTRA_REQUIRE_WGPU=1` so unavailable
WGPU cannot silently skip the probe. For hardware measurements, first establish
an integrated/discrete classification through the validation procedure and retain
the actual probe adapter output. The availability flag alone does not establish
hardware execution. Run one case per process:

```bash
mkdir -p target/benchmark-results/group-canvases
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
memory measurements. The benchmark runner currently records engine counters;
process-memory captures need external tooling and must not be presented as a
built-in report field. Missing measurements are unavailable, not zero.

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
