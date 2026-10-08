# Performance

The [CPU efficiency report](cpu-efficiency.md) describes the current CPU work
reuse mechanisms and their constraints.

Measure a named stage, not an undifferentiated "render time". Vestra reports preparation timing for semantic validation, preflight, plan compilation, decode, audio analysis and backend initialization. Render timing then separates track evaluation, frame rendering, encoder writes/finalization, output publication, and WGPU upload/encode/submission/readback work where applicable.

For every result record the revision, command, scene/project, output size/frame rate/quality, frame count, CPU/OS, FFmpeg build, requested renderer preference, actual selected renderer, graphics backend, adapter name and classification. Record whether assets and renderer resources were cold or already prepared. A `PreparedProject` may make later frame/video operations much cheaper than a one-shot render, and that is expected rather than a comparable baseline.

Use separate measurements for planning/preparation, CPU frame rendering, hardware WGPU frame rendering, software WGPU fallback, audio execution, video decode, encoding and end-to-end render. Encoder, muxer and publication time can dominate a short scene; do not call that a renderer regression without a frame-only comparison. Likewise, a media decode change needs a controlled decoder workload.

Do not compare llvmpipe/Lavapipe results with hardware GPU numbers or call them GPU performance. First run [GPU validation](gpu-validation.md) and capture the actual adapter. Keep hardware and software adapter results separate.

## Measure and compare

Capture a [local baseline](../../benchmarks/README.md) with raw
samples and environment metadata before comparing optimizations. Personal
machine captures are not portable performance targets.

Run from the repository root with Python 3.11+, Cargo, Git, and FFmpeg on PATH.
The Rust build also needs the native tools for bundled FFmpeg documented in the build guide. No Python packages are required by
the benchmark script.

```bash
just benchmark-smoke target/benchmark-results/smoke
just benchmark target/benchmark-results/before
# Make one optimization, then measure with the same machine and settings.
just benchmark target/benchmark-results/after
just benchmark-compare target/benchmark-results/before/suite.json target/benchmark-results/after/suite.json
```

These recipes run the existing benchmark script through `uv run --no-project`.
Enter `nix develop` to get Just and the native tools. Use
`just benchmark OUTPUT hardware-wgpu` for a canonical hardware suite. The script
remains directly callable when additional options such as `--executable` are
needed.

Use a new output directory for each run. The runner builds the existing
`animation_effects` release benchmark and executes scenarios serially. Each
directory contains per-scenario logs and raw JSON records. It writes `suite.json`
only after every scenario succeeds and the source fingerprint remains unchanged.
An interrupted or failed run retains its diagnostic files but has no completed
suite. `--executable /absolute/path` skips building and records that the caller
supplied the executable; the caller must ensure it matches the current sources.

The versioned definition in `benchmarks/suites.json` fixes the scenario order,
resolution, warmups, and sample count. Smoke uses 128×72, no warmup, and one
sample to check execution. Canonical uses 1280×720, one warmup, and five samples.
Both cover video decode, three concurrent moving-video layers, effects,
geometric masks, track mattes, nested groups, seeded particles with bloom,
blend modes, and the twelve-second production edit.

The production edit uses deterministic synthetic moving footage in a three-shot
timeline with overlapping dissolves, saturation correction, animated grouped
titles, and a twelve-second audio bed. It exercises editorial operations without
requiring downloaded footage. It does not model the decode cost of every camera
codec. Media generation and hashing happen outside measured render intervals.
FFV1 and WAV fixtures use bit-exact output; particle fixtures use explicit seeds.

Comparison accepts individual scenario files or complete suites. It checks
workload content identities, suite definitions, environment, frame counts,
timing scope, and actual backend/adapter before reporting median timing and
resource deltas. `--json` emits the same deltas for automation. A negative timing
delta is faster. Resource counters need interpretation: more cache hits may be
good, while more retained bytes may be an unwanted tradeoff. A zero baseline
reports an absolute delta with no percentage.

Read the raw samples as well as the median. Five samples describe this run;
they do not establish statistical significance. Repeat baseline and candidate
runs when differences are small or ranges overlap. Use the same power mode,
thermal conditions, background load, toolchain, FFmpeg build, feature set, and
environment controls. Keep an optimization only when the intended stage improves
repeatably, resource tradeoffs are acceptable, and correctness checks still pass.
Otherwise revert the optimization and retain the reports as evidence.

## Measurement scope

Every measured sample constructs a new editor and prepares the project again.
The wall interval includes project loading and the end-to-end render, including
encoding and publication. OS page caches are uncontrolled and often warm after
the warmup. This is not a disk-cold measurement or a prepared-project measurement.
Raw records retain every serialized `RenderResult`, including stage timings,
resource counters, warnings, and actual backend selection. Millisecond stage
timings may be zero for short work; concurrent frame work can exceed wall time.
Resource bytes are engine counters and estimates, not process RSS measurements.

Keep the existing internal benchmarks for stage isolation:

```bash
VESTRA_RENDER_BENCH=1 cargo test --release -p vestra --no-default-features --features cpu preparation_matrix -- --nocapture
VESTRA_RENDER_BENCH=1 cargo test --release -p vestra --no-default-features --features cpu effect_scaling_matrix -- --nocapture
VESTRA_RENDER_BENCH=1 cargo test --release -p vestra --no-default-features --features cpu random_access_matrix -- --nocapture
```

These gated tests write under `target/benchmark-results` and retain their own
prepared/null-sink measurement format. They complement the end-to-end suite;
their timings must not be compared directly with its one-shot wall times.

Progress is disabled in benchmarks. Live terminal progress continues to report
smoothed FPS and ETA, with elapsed time on completion. Detailed tracing and
external profilers remain opt-in; collect profiles in separate runs because
instrumentation can change timings.

## Hardware WGPU

After the platform preflight in [GPU validation](gpu-validation.md), run:

```bash
python scripts/benchmark.py run --suite canonical --backend hardware-wgpu --output target/benchmark-results/hardware-before
```

The runner requires an actual WGPU result with an integrated or discrete adapter
for every measured sample. CPU fallback, software WGPU, and unknown adapter
classes fail the suite. CPU and hardware runs have separate baselines. If
hardware is unavailable, keep that validation explicitly outstanding; software
WGPU results cannot substitute for it.

## Focused palette and dither workloads

The [focused stylization suite](../../benchmarks/README.md) isolates moving
video with no effect, Palette Map, and Ordered Dither. Run
`python scripts/benchmark.py run --suite stylization-1080p --backend cpu --output target/benchmark-results/stylization-before`
for 1920×1080 at 30 fps, 90 frames, one warmup and three samples per workload.
Use `--suite stylization-smoke` for execution checks and
`--backend hardware-wgpu` for a separately verified hardware baseline.
All three scenarios generate identical full-frame `testsrc2` video without
additional graphics or audio. Generation is excluded from measured intervals.

The focused suite uses the existing record schema, stage/resource counters,
source-stability gate, and comparison compatibility checks. Canonical and
smoke suites continue to use their original ten scenarios. Suite-local
scenario selection is recorded explicitly in `suite.json`; comparisons reject
changed scenario sets, dimensions, or sample/warmup counts. A 4K correctness
check can override dimensions when running the same benchmark directly, but
must be reported separately from the versioned 1080p measurements.
