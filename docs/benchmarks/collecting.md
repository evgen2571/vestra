---
title: Run benchmarks
description: Run the maintained benchmark suites on Windows, macOS or Linux.
---

# Run benchmarks

The collector uses bundled footage, builds the selected checkout, validates
rendered output, and writes results locally. The first
build can take tens of minutes; leave the terminal open and the computer plugged in.

## Prepare the computer once

Install uv, Rust, Git and the native tools for your platform. Python and benchmark
packages are managed automatically by uv. Source builds need a compiler; this
is one collection command **after setup**, not a prerequisite-free installation.

Use the maintained platform instructions:

- [Windows x64: Visual Studio developer terminal, LLVM and media tools](https://github.com/evgen2571/vestra/blob/develop/benchmarks/COLLECTING.md#windows-x64)
- [macOS: Xcode tools, LLVM and media tools](https://github.com/evgen2571/vestra/blob/develop/benchmarks/COLLECTING.md#macos-apple-silicon-or-intel)
- [Linux: compiler, LLVM and media tools](https://github.com/evgen2571/vestra/blob/develop/benchmarks/COLLECTING.md#linux)

## Clone and run

Choose the exact commit to measure. After cloning, `git checkout COMMIT` selects
it; replace `COMMIT` with the desired hash. Use the same commit on every computer
in a comparison, and keep the checkout unchanged during measurement.

```bash
git clone https://github.com/evgen2571/vestra.git
cd vestra
git checkout COMMIT
uv run --no-project --python 3.13 scripts/collect-benchmarks.py
```

The default run includes CPU smoke checks, six representative 640 × 360
workloads, the three matched MoviePy timelines, and hardware GPU measurements
when an eligible adapter is found. Hour-long rendering and decoder-cache
experiments are optional.

The terminal shows task starts, completions and elapsed time. During long worker
commands it prints an activity update every fifteen seconds. Builds, input
generation and validation are outside the render measurements.

For an NVIDIA Windows PC, automatic discovery tries the platform's available
APIs; on macOS it tries Metal. The report records the device and actual backend.
An unavailable or software adapter is labelled explicitly. It does not become a
hardware result through CPU fallback.

## Read the results

Results are written under `target/benchmark-results/`. Open `summary.md` in the
run directory for the revision, machine, execution and validation status,
render times and sample ranges. Raw records and logs support investigation.

Required failures return a nonzero exit status and preserve partial diagnostics.
Ctrl+C stops owned worker processes and retains completed evidence.

## Optional checks

```bash
# Check installed tools before committing time to the build.
uv run --no-project --python 3.13 scripts/collect-benchmarks.py --preflight-only

# A small CPU execution check; these timings are not performance results.
uv run --no-project --python 3.13 scripts/collect-benchmarks.py --smoke --backend cpu
```

For timeouts, strict hardware selection, output folders, cache or endurance
profiles, use the [full collection reference](https://github.com/evgen2571/vestra/blob/develop/benchmarks/COLLECTING.md#optional-profiles).
Windows and macOS measurements require actual native runs; Linux tests and
mocked platform checks do not prove those systems work.
