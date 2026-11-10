# Benchmarks

See what Vestra renders, how long it takes, and how to collect comparable results
on another machine.

**[See results](../docs/benchmarks/index.md)** ·
**[Explore workloads](../docs/benchmarks/workloads.md)** ·
**[Run benchmarks](COLLECTING.md)**

## Collect results on your PC

After the [one-time native tool setup](COLLECTING.md#install-tools-once), run from
the checkout root:

```bash
uv run --no-project --python 3.13 scripts/collect-benchmarks.py
```

The collector builds this checkout, prepares the bundled footage, runs the short CPU
and MoviePy comparisons, and tests hardware WGPU when an eligible adapter is
available. It validates the outputs and writes results under
`target/benchmark-results/`. Each run includes a readable `summary.md`,
machine/revision information, raw samples and diagnostics.

The first build can take tens of minutes. Hour-long endurance and decoder-cache
experiments are optional; they are not part of the default collection. Windows
and macOS need native build tools: the single command does not install those.

## Choose a workflow

| You want to… | Start here |
| --- | --- |
| Understand the published measurements | [Results and context](../docs/benchmarks/index.md) |
| See each scene and its suite membership | [Workload gallery](../docs/benchmarks/workloads.md) |
| Run on Windows, macOS or Linux | [Run benchmarks](COLLECTING.md) |
| Measure a code change on the same machine | [Performance methodology](../docs/development/performance.md) |
| Investigate a stage, effect, cache or decoder | [Specialist reference](REFERENCE.md) and [performance probes](../docs/development/performance-probes.md) |
| Run the local-footage MoviePy workflow | [Comparison reference](comparisons/moviepy/README.md) |

## Local regression comparison

```bash
just benchmark-smoke target/benchmark-results/smoke
just benchmark target/benchmark-results/before
# Make one optimization, keeping the machine and measurement settings matched.
just benchmark target/benchmark-results/after
just benchmark-compare target/benchmark-results/before/suite.json target/benchmark-results/after/suite.json
```

Use new output directories. Smoke proves execution only. The strict comparator
rejects incompatible workloads, environments and backend selections. Keep
generated results under ignored `target/benchmark-results/`; cross-machine
captures describe their hosts and cannot establish a code regression.

The source of truth is [suites.json](suites.json), the
[Rust workload builder](../crates/vestra/benches/animation_effects.rs), and the
[shared MoviePy timelines](comparisons/moviepy/workloads.py).
