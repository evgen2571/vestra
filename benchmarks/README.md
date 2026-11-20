# Benchmarks

[Published results](../docs/benchmarks/index.md) · [Workload previews](../docs/benchmarks/workloads.md) · [Setup guide](../docs/benchmarks/collecting.md)

## Run on your machine

After the [one-time setup](../docs/benchmarks/collecting.md#install-tools-once), run from the repository root.

```bash
uv run --no-project --python 3.13 benchmarks/collect.py
```

The collector runs the short CPU and MoviePy suites. It also tests WGPU when
eligible hardware is available. Open `summary.md` in the new run directory
under `target/benchmark-results/`. A first build and the Full HD runs can take
an hour or more.

## Local regression comparison

| Goal | Start here |
| --- | --- |
| Check that the runner works | `just benchmark-smoke target/benchmark-results/smoke` |
| Measure a code change | [Run and compare a matched baseline](../docs/development/performance.md#measure-and-compare) |
| Compare Vestra and MoviePy | [MoviePy workflow](comparisons/moviepy/README.md) |
| Investigate effects, audio or decoding | [Specialist reference](REFERENCE.md) |

## Source layout

| File | Role |
| --- | --- |
| [collect.py](collect.py) | Run the portable collector |
| [benchmark.py](benchmark.py) | Run suites and compare results |
| [suites.json](suites.json) · [projects/](projects/) | Suite definitions and JSON projects |
| [rust/](rust/) · [MoviePy workloads](comparisons/moviepy/workloads.py) | Workload builders and probes |
