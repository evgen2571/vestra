# Performance

Compare a local baseline and candidate on the same machine, then use a focused
probe to explain a stage or resource cost. Smoke timings prove execution only.
For an existing regression, [choose a focused investigation](#choose-a-focused-investigation).

## Measure and compare

Run from the repository root with Python 3.11+, Cargo, Git, FFmpeg and the
native build tools from the [contributor setup](https://github.com/evgen2571/vestra/blob/develop/CONTRIBUTING.md).
`nix develop` supplies Just and those tools. The recipes use
`uv run --no-project`; the benchmark script needs no additional Python packages.

```bash
just benchmark-smoke target/benchmark-results/smoke
just benchmark target/benchmark-results/before
# Make one optimization, then use the same machine and settings.
just benchmark target/benchmark-results/after
just benchmark-compare target/benchmark-results/before/suite.json target/benchmark-results/after/suite.json
```

For hardware, first [discover and verify the adapter](gpu-validation.md), then
use `just benchmark OUTPUT hardware-wgpu` with a separate baseline. Every
sample must select an integrated or discrete adapter; CPU fallback, software
WGPU and unknown classes fail the hardware suite.

1. **Capture a complete baseline.** Use a new output directory for each run.
   The runner builds the `animation_effects` release benchmark and executes
   scenarios serially. Per-scenario logs and raw JSON remain available if a
   run fails. `suite.json` appears only after all scenarios succeed and the
   source fingerprint remains unchanged.
2. **Measure a matched candidate.** Keep the suite, worker count, feature set,
   toolchain, FFmpeg build, power mode, thermal conditions, background load and
   environment controls consistent. Do not edit sources during a suite.
3. **Inspect the comparison and raw samples.** The comparison rejects changed
   workload identities, suite definitions, environments, frame counts, timing
   scopes and actual backend/adapter selections. It accepts scenario files or
   whole suites; `--json` returns the same deltas for automation.
4. **Confirm the result.** A negative timing delta is faster. Five samples
   describe one run, not statistical significance; repeat paired runs when
   differences are small or ranges overlap. Keep a change only when its intended
   stage improves repeatably, resource costs are acceptable and correctness
   checks pass, including actual rendered pixels.

The versioned definition in `benchmarks/suites.json` fixes scenario order and
settings:

| Suite | Resolution | Warmups / samples | Purpose |
| --- | --- | --- | --- |
| `smoke` | 128×72 | 0 / 1 | Execution check |
| `canonical` | 1280×720 | 1 / 5 | End-to-end baseline |

Both cover decode, concurrent moving-video layers, effects, masks/mattes, nested
groups, seeded particles with bloom, blend modes and a twelve-second production
edit. The edit uses deterministic synthetic footage, dissolves, correction,
animated grouped titles and audio; it does not represent every camera codec.
Media generation and hashing are outside measured intervals. FFV1/WAV fixtures
use bit-exact output and particles use explicit seeds.

The script is also [directly callable](https://github.com/evgen2571/vestra/blob/develop/benchmarks/README.md).
`--executable /absolute/path` skips building and records a caller-supplied
executable; the caller must ensure it matches the sources. Keep raw local
reports for review, without committing personal machine captures as portable
targets. A changed workload or environment needs a new named baseline.

## Measurement scope

Every canonical measured sample creates an editor and prepares the project
again. Its wall interval includes project loading, rendering, encoding and
publication. Warmup often warms OS page caches; these runs are neither
disk-cold nor prepared-project measurements.

Record these details for every measurement:

| Record | Required context |
| --- | --- |
| Workload | Revision, command, scene/project, dimensions, frame rate, quality and frame count |
| Environment | CPU/OS, toolchain, FFmpeg build, features, worker count and environment controls |
| Backend | Requested preference, actual renderer, graphics API, adapter name and classification |
| Resource state | Cold or reused assets/renderer; one-shot, prepared-frame or prepared-video operation |

Measure the stage that changed. Reports separate validation/preflight,
compilation, decode, audio analysis and backend initialization from evaluation,
frame rendering, encoder writes/finalization and publication. WGPU also reports
upload, command encoding/submission, readback waits and row repacking. A short
encoded render can be dominated by the encoder; use a frame-only comparison
before attributing that difference to the renderer.

Raw records retain every serialized `RenderResult`, including warnings, stage
timings, resource counters and actual backend. Short stages may round to zero
milliseconds. Concurrent frame work and [frame-write overlap](media-pipeline.md)
mean summed stage times need not equal wall time. Progress is disabled in
benchmarks; tracing and profilers belong in separate runs because they change
timings.

Keep CPU, software WGPU and hardware WGPU results separate; follow
[GPU validation](gpu-validation.md) for the required evidence. Compare repeated
`PreparedProject` operations separately from one-shot renders because reuse
intentionally avoids preparation.

## Choose a focused investigation

<span id="isolate-preparation-effects-or-random-access"></span>
<span id="hardware-wgpu"></span>
<span id="local-canvas-allocation-probe"></span>
<span id="focused-palette-and-dither-workloads"></span>
<span id="cache-working-set-and-native-worker-diagnostics"></span>
<span id="native-worker-sweep"></span>

| Question | Procedure | Output / scope |
| --- | --- | --- |
| Which preparation stage, effect or random access changed? | [CPU stage probes](performance-probes.md#isolate-preparation-effects-or-random-access) | Prepared/null-sink SDK records |
| Is hardware frame work or readback expensive? | [Local-canvas WGPU probe](performance-probes.md#local-canvas-allocation-probe) | Prepared-backend latency, staged throughput and texture bytes |
| What does palette/dither add to moving video? | [Focused stylization suites](https://github.com/evgen2571/vestra/blob/develop/benchmarks/README.md) | Matched no-effect, palette and dither end-to-end records |
| Is memory growing or allocation work repeated? | [Sustained profiling](performance-probes.md#allocation-and-sustained-render-profiling) | External process time series alongside engine counters |
| Does a cache budget change cost or pixels? | [Cache working sets](performance-probes.md#cache-working-set-and-native-worker-diagnostics) | Timing JSON, partition traces or cache-disabled pixel comparisons |
| How do native decoder workers scale? | [Native worker sweep](performance-probes.md#native-worker-sweep) | External FFmpeg references; separate oracle and timing runs |

The three-workload `stylization-1080p` suite uses the same generated full-frame
`testsrc2` video for no effect, Palette Map and Ordered Dither: 1920×1080,
30 fps, 90 frames, one warmup and three samples. Use `stylization-smoke` to
check execution. Other effect/control suites and their exact workloads live in
the linked benchmark reference. Report 4K correctness separately from versioned
1080p measurements.

## Interpret resource changes

<span id="allocation-and-sustained-render-profiling"></span>

Engine byte counters describe named storage and estimates, **not process RSS or
device-resident memory**. Cache hits and copies do not cover every allocation.
Fewer allocations may retain more memory; more cache hits do not prove a speedup.
A zero baseline gives an absolute delta with no percentage.

Use [CPU accounting](architecture/cpu-renderer.md#memory-accounting) to interpret
retained capacity, per-owner peaks and worker estimates. The
[profiling procedure](performance-probes.md#allocation-and-sustained-render-profiling)
explains external process-tree sampling, sustained runs and missing metrics.
Worker count trades throughput against scratch and decoder resources, so keep
it matched when comparing both speed and memory.

<span id="cpu-renderer-efficiency"></span>
<span id="scratch-and-sparse-convolution"></span>
<span id="measure-cost-and-resource-tradeoffs"></span>

Existing work avoidance includes constant/identity normalization, exact-transform
sampling paths and static-layer reuse. [CPU fast paths](architecture/cpu-renderer.md#fast-paths-and-operation-metrics)
describe eligibility and operation counters, including
[sparse convolution](architecture/cpu-renderer.md#sparse-convolution). These
optimizations preserve owned output frames,
ordering, masks and cache invalidation. Validate pixels and cache-disabled
references before accepting a speedup; a counter change alone is insufficient.
