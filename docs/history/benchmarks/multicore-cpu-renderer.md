# Multi-Core CPU Renderer Audit

Status: complete; release benchmark and repository validation passed.

## Previous bottleneck

The CPU backend previously submitted and rendered through one worker slot during
normal production use. The accepted staged engine already supplied bounded
submission, completion polling, out-of-order buffering, ordered writes, and
cancellation; the remaining renderer work was persistent inter-frame workers.

## Final architecture

```text
render engine
    -> capacity / in_flight staged scheduler
    -> CpuBackend
    -> N persistent workers
    -> worker-local CpuWorkerState
    -> compositor::compose()
```

Workers render whole frames serially. `Arc<DecodedAssets>` is shared and
immutable. Each worker exclusively owns its prepared crop cache, static-layer
cache, three-surface `EffectSurfacePool`, and counters. Command and completion
channels remain bounded; no global compositor lock or per-frame thread spawn is
used.

## Automatic worker policy

The internal deterministic helper uses:

- logical CPU availability from `available_parallelism()`;
- one reserved logical CPU when availability is greater than one;
- `MAX_AUTO_CPU_WORKERS = 8`;
- `ESTIMATED_LIVE_RGBA_FRAMES_PER_WORKER = 6`;
- `AUTO_CPU_FRAME_MEMORY_BUDGET_BYTES = 512 MiB`.

For a frame, `width * height * 4` is calculated with checked `u64`
arithmetic. The estimate is six such frames per worker. The final count is
`max(1, min(cpu_limit, memory_budget / estimated_worker_bytes, 8))`, with
overflow reducing the memory-derived limit to one. Worker count remains an
internal renderer decision and is not part of project JSON, schema, CLI, or
SDK configuration.

## Cache budget partitioning

Each worker receives quotient/remainder partitions of
`plan.limits.maximum_cache_bytes`, independently for crop and static-layer
caches. The parts sum exactly to the configured class budget and differ by at
most one byte. Aggregated crop gauges and counters sum worker snapshots; static
gauges and counters do likewise. Decoded-image metrics remain counted once.

## Ordering, determinism, and lifecycle

The backend may surface completions out of order. The engine's existing
`BTreeMap` writes frames in frame-number order. `flush()` drains work while
leaving healthy workers, caches, and surface pools reusable; `abort()` and
`Drop` join every worker. Cancellation stops new submissions and uses
cancellable polling before aborting active work.

Worker panics remain structured `CPU-WORKER-PANIC` diagnostics with the exact
frame number. Both normal-loop and final-drain failures prefer
`failed_frame_number()` over the last project frame fallback.

## Benchmark methodology

The direct benchmark is an ignored release-mode test in
`crates/video-editor-render/src/cpu/backend.rs`:

```bash
cargo test --release -p video-editor-render cpu_parallel_scaling_benchmark \
  --all-features -- --ignored --nocapture
```

It evaluates the same 1920×1080 animation/effects workload for 60 frames,
warms each persistent backend once, then measures 1, 2, 4, and automatic
workers. It reports wall time, effective FPS, aggregate worker render time,
peak in-flight frames, cache gauges, and retained scratch bytes.

The end-to-end benchmark reports the actual backend pipeline depth separately
from the requested WGPU pipeline-depth environment variable:

```bash
VIDEO_EDITOR_BENCH_BACKEND=cpu VIDEO_EDITOR_BENCH_WIDTH=1920 \
VIDEO_EDITOR_BENCH_HEIGHT=1080 VIDEO_EDITOR_BENCH_WARMUPS=1 \
VIDEO_EDITOR_BENCH_SAMPLES=1 cargo bench -p video-editor \
  --bench animation_effects -- --nocapture
```

## Benchmark results

Fill this table from the exact release command output. Do not infer CPU
utilization or RSS when optional host tools were not run.

| Workers | Resolution | Frames | Wall ms | Effective FPS | Aggregate frame render ms | Peak in-flight | CPU util | Peak RSS |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| 1 | 1920×1080 | 60 | 12,372.512 | 4.85 | 12,368.447 | 1 | not measured | not measured |
| 2 | 1920×1080 | 60 | 6,215.874 | 9.65 | 12,405.502 | 2 | not measured | not measured |
| 4 | 1920×1080 | 60 | 3,181.688 | 18.86 | 12,671.340 | 4 | not measured | not measured |
| auto (8) | 1920×1080 | 60 | 2,077.725 | 28.88 | 15,989.852 | 8 | not measured | not measured |

The direct benchmark measured clear scaling: 2 workers were 1.99× faster,
4 workers 3.89× faster, and auto (8 workers) 5.95× faster than one worker on
this host. CPU utilization and RSS were not collected by the direct harness.

The end-to-end release benchmark was also run with the canonical
`animation_effects` path at 1920×1080, CPU backend, one measured sample, and no
warmups:

| Backend workers | Frames | Wall ms | Effective FPS | Aggregate frame render ms | Peak in-flight | Actual pipeline depth | Cache peak/current bytes | Scratch retained bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| auto (8) | 144 | 6,938 | 20.76 | 52,760 | 8 | 8 | 0 / 0 | 199,065,600 |

This path also reported `requested_wgpu_pipeline_depth=3` separately from the
actual CPU backend depth of 8. CPU utilization and RSS were not collected.

## Known scaling limits

The policy is deliberately conservative: each worker owns a fixed scratch
workspace and receives only a partition of each cache class. Memory bandwidth,
FFmpeg encoding, and small or highly cacheable workloads can limit scaling.
Performance acceptance is therefore based on measured wall time and effective
FPS on a suitable host, not a brittle unit-test speedup threshold.

## Acceptance conclusion

The release direct benchmark and end-to-end benchmark ran successfully. The
focused renderer and engine suites, workspace validation, schema comparison,
and clippy gates passed. No known blocker remains for the Multi-Core CPU
Renderer phase.
