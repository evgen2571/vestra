# Vestra and MoviePy v2 comparison

Three matched video edits at 1920 × 1080 and 60 fps. Timings cover startup,
scene setup, decoding, rendering and encoding. Input preparation and output
verification run outside the measured interval. [Published results and methodology](../../../docs/benchmarks/vestra-vs-moviepy.md)

## Reproduce

Run from the repository root with the [native development environment](../../../CONTRIBUTING.md#2-set-up-a-checkout)
and FFmpeg/FFprobe. On Nix, use `nix develop .#comparison` for the NumPy runtime
and GPU loaders.

```bash
uv sync --locked --extra benchmark --extra dev
uv run --locked --extra benchmark --extra dev python benchmarks/benchmark.py moviepy run \
  --output target/moviepy-cpu
```

The default run uses the bundled footage, one warmup and five samples per engine
for each edit. Use a fresh output directory for each run.

| Scenario | Edit |
| --- | --- |
| `basic` | Resize and encode one shot. |
| `editing` | Dissolve between three shots with animated framing and brightness adjustment. |
| `layered` | Composite a background and two moving overlays, one with an ellipse mask. |

Use `--scenarios editing` to run one edit, or see the
[shared timeline definitions](workloads.py) for exact settings.

## Verify and report

```bash
uv run --locked --extra benchmark --extra dev python benchmarks/benchmark.py moviepy verify \
  target/moviepy-cpu --output target/moviepy-visuals
uv run --locked --extra benchmark --extra dev python benchmarks/benchmark.py moviepy report \
  target/moviepy-cpu --verification target/moviepy-visuals/metrics.json
```

Inspect the generated contact sheets and frame metrics. The verifier samples
frames around edits and checks layers against source frames where possible.
Sampling and rounding can differ between the engines.

<details>
<summary>Quick smoke run</summary>

```bash
uv run --locked --extra benchmark --extra dev python benchmarks/benchmark.py moviepy run \
  --width 320 --height 180 --warmups 0 --samples 1 --output target/moviepy-smoke
```

Use this to check setup and execution. Use the standard run for measurements.

</details>

<details>
<summary>Hardware WGPU</summary>

Use the same graphics API environment for CPU and hardware captures you plan
to verify together. Discover the adapter first and replace `vulkan` with the
selected API.

```bash
just wgpu-list
VESTRA_WGPU_BACKEND=vulkan uv run --locked --extra benchmark --extra dev \
  python benchmarks/benchmark.py moviepy run --engines hardware-wgpu \
  --output target/moviepy-hardware
```

Pass both capture paths to `verify` and `report` to include hardware results.
The hardware run requires an eligible adapter. Decode and H.264 encoding
still run on the CPU.

</details>

<details>
<summary>Use different footage</summary>

```bash
uv run --locked --extra benchmark --extra dev python benchmarks/benchmark.py moviepy prepare \
  --assets /path/to/footage --inputs target/moviepy-inputs
```

Use the same `--inputs target/moviepy-inputs` on both `run` and `verify`.
Collect a new baseline when you change the source footage.

</details>

## Evidence and failures

`comparison.json` records sample times, environment, the actual adapter and
output checks. Failed runs leave logs under their output directory. Keep local
reports under `target/` and check them for machine paths before sharing.
[Measurement and reporting details](../../../docs/benchmarks/collecting.md#read-and-share-results)
