# Performance measurement foundation verification

The foundation reuses `crates/vestra/benches/animation_effects.rs` and existing
engine timing/resource reports. The prepared/null-sink benchmarks remain
available for stage isolation. No engine, SDK, binding, CLI report schema, or
render hot-path code changed.

| Requirement | Evidence |
| --- | --- |
| Representative workloads | Ten scenarios in `benchmarks/suites.json`, all rendered in smoke and canonical CPU suites |
| Video-heavy rendering | Three simultaneous moving-video layers, with actual decode counters verified |
| Production-style edit | Twelve seconds, three shots, two dissolves, grading, animated grouped titles, audio; 360 frames verified |
| Effects, masks/mattes, nesting, particles, compositing | Named canonical records for each, with expected frame counts |
| Deterministic assets and projects | Bit-exact generated FFV1/WAV, explicit particle seeds, content-based workload identities; identities match across independent runs |
| Structured results | Every measured `RenderResult`, wall timing, environment/backend/revision data, source and executable fingerprints |
| Repeatable suites | Fixed smoke/canonical definitions, serial execution, complete-suite publication, source-change detection |
| Timing and resource comparison | Median deltas for 1,020 metrics; independent suite comparison succeeded; incomplete/mislabeled suites rejected |
| Canonical baseline | `benchmarks/baselines/cpu-v1.json`: ten workloads, five samples each, one warmup, 1280×720 CPU |
| Progress and profiling separation | Benchmark requests disable progress; serialization/hashing stays outside measured renders; profiling remains opt-in |
| Measure/optimize/compare/keep-or-revert workflow | `docs/development/performance.md`, with noise, cache-scope, and resource interpretation guidance |

## Checks performed

- CPU release benchmark build completed successfully, including automatic Cargo
  executable discovery through the suite runner.
- 26 focused Python tests passed, including real release renders, whole-suite
  recording, asset regeneration, production audio, and comparison failures.
- Two independent smoke suites completed and compared all ten workload identities
  and 1,020 timing/resource metrics. These runs verify reproducibility and the
  comparison workflow; their timings are not optimization evidence.
- Canonical CPU suite completed all 50 measured renders, plus warmups. Each
  record's dimensions, sample count, frame count, actual backend, and warnings
  were inspected. Its self-comparison returned zero deltas for all 1,020 metrics.
- Focused release Clippy for the benchmark passed. Existing CPU-only warnings in
  `vestra-render` concern unused GPU-related imports and geometry fields.
- Rust formatting, focused Ruff checks, and `git diff --check` passed.

The build host lacked NASM. A Debian NASM package was unpacked under
`/tmp/vestra-bench-tools/extracted`, whose `usr/bin` was added to PATH for builds.
System package installation was unavailable because of a pre-existing broken
`libclang-dev` dependency. No project dependency changes were needed.

## Validation limits

This is a CPU baseline. The host exposes no `/dev/dri`, `/dev/dxg`, or
`nvidia-smi`; hardware-WGPU measurement remains unavailable. The hardware suite's
adapter-class checks are unit-tested, but no hardware render is claimed and no
software WGPU result substitutes for one. Capture a separate hardware baseline
on a machine with a confirmed integrated/discrete adapter before GPU optimization.

Live terminal progress already supplies smoothed FPS and ETA and reports elapsed
time on completion. Its behavior was preserved. Resource counters are engine
measurements/estimates, not process RSS. Synthetic video assets exercise editorial
and decode operations without claiming coverage of every production codec.
