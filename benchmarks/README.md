# Benchmarks

See what Vestra renders, how long it takes, and how to collect comparable results
on another machine.

**[See results](../docs/benchmarks/index.md)** ·
**[Explore workloads](../docs/benchmarks/workloads.md)** ·
**[Run benchmarks](../docs/benchmarks/collecting.md)**

## Collect results on your PC

After the [one-time native tool setup](../docs/benchmarks/collecting.md#install-tools-once), run from
the checkout root:

```bash
uv run --no-project --python 3.13 benchmarks/collect.py
```

The collector builds this checkout, prepares the bundled footage, runs the short CPU
and MoviePy comparisons, and tests hardware WGPU when an eligible adapter is
available. It validates the outputs and writes results under
`target/benchmark-results/`. Each run includes a readable `summary.md`,
machine/revision information, raw samples and diagnostics.

The first build can take tens of minutes; repeated 1080p60 measurements can take
an hour or more. Hour-long endurance and decoder-cache
experiments are optional; they are not part of the default collection. Windows
and macOS need native build tools: the single command does not install those.

## Choose a workflow

| You want to… | Start here |
| --- | --- |
| Understand the published measurements | [Results and context](../docs/benchmarks/index.md) |
| See each scene and its suite membership | [Workload gallery](../docs/benchmarks/workloads.md) |
| Run on Windows, macOS or Linux | [Run benchmarks](../docs/benchmarks/collecting.md) |
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
[Rust workload builder](rust/animation_effects.rs), and the
[shared MoviePy timelines](comparisons/moviepy/workloads.py).

## Source layout

All benchmark programs and workload definitions live in this directory.
Start here when adding or investigating a measurement:

| Location | Responsibility |
| --- | --- |
| `collect.py`, `collector/` | Portable collection entry point, host setup, fixtures, cache and sustained runs. |
| `benchmark.py`, `process.py` | Suite runner, strict result comparison and process-tree sampling. |
| `suites.json`, `projects/` | Suite settings and canonical project fixtures. |
| `comparisons/moviepy/` | Matched MoviePy workloads and output validation. |
| `rust/animation_effects.rs`, `rust/support/` | End-to-end Cargo benchmark owned by the `vestra` package. |
| `rust/sdk/` | SDK preparation, effects, random access and cache probes. |
| `rust/cpu/` | Prepared-work and native-decoder worker probes. |
| `rust/media/` | Decoder access probes and the collector's `benchmark_access` example target. |
| `python-abi.py` | Installed-wheel Python ABI timing probe. |
| `render-previews.py`, `render-charts.py` | Documentation delivery media; their timings are not benchmark evidence. |

Private probes are wired into their owning crate's existing test modules with
`#[path]`; this keeps internal access without exporting production APIs. Cargo
commands and test names are unchanged. Smaller measurements embedded in ordinary
correctness tests remain beside the behavior they check; their commands are in
[the probe reference](../docs/development/performance-probes.md).
Published results and the single collection procedure live in `docs/benchmarks/`.
Generated captures live only under ignored `target/benchmark-results/`.
