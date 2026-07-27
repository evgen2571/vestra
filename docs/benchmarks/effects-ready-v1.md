# Effects-ready v1 benchmark recipe

Run the harness with the CPU backend. It defaults to the basic-colour scene:

```bash
TMPDIR=target/tmp VIDEO_EDITOR_BENCH_BACKEND=cpu cargo bench --bench animation_effects
```

Select an advanced scenario with `VIDEO_EDITOR_BENCH_SCENARIO`. The available
scenarios are `baseline`, `basic_colour`, `gaussian_small`, `gaussian_large`,
`glow`, `sharpen`, `directional_blur`, `motion_blur`, `blend_modes`,
`global_post`, `impact`, `heavy_impact`, `transitions`, `zoom_blur`, and
`combined`.

```bash
TMPDIR=target/tmp VIDEO_EDITOR_BENCH_BACKEND=cpu \
  VIDEO_EDITOR_BENCH_SCENARIO=combined cargo bench --bench animation_effects
```

The harness defaults to 720x1280 with five warmups and five measured renders.
For a quick, explicitly non-comparable development check, override all four
parameters, for example `VIDEO_EDITOR_BENCH_WIDTH=320`,
`VIDEO_EDITOR_BENCH_HEIGHT=180`, `VIDEO_EDITOR_BENCH_WARMUPS=1`, and
`VIDEO_EDITOR_BENCH_SAMPLES=1`. The harness reports resolution, frame count, selected backend,
median wall and render timing, frame composition time, encoding time, and
cache/decoded-resource peaks. Use the original color-only fixture as the
baseline. To measure an advanced chain, replace its clip effect array with the
ordered chain from `examples/projects/effects-ready-v1.json` and record the
same fields. Both CPU and WGPU support the advanced scenarios. On a compatible
adapter, run the same matrix with `VIDEO_EDITOR_BENCH_BACKEND=wgpu` and
`VIDEO_EDITOR_REQUIRE_WGPU=1`; record the selected adapter and the command,
submission, readback, and row-repack timings printed by the harness. Do not
compare Lavapipe measurements with a discrete GPU as performance evidence.

Gaussian blur, glow, sharpen, and combined chains are expected to dominate
frame composition time. Record the median wall time, frame-render time, and
effective FPS for each scenario alongside the baseline. The renderer reuses
its intermediate surfaces for ordered chains; it does not allocate one canvas
per pass.

## Recorded CPU development matrix (2026-07-23)

Command (run once per scenario):

```bash
VIDEO_EDITOR_BENCH_BACKEND=cpu VIDEO_EDITOR_BENCH_SCENARIO=<scenario> \
VIDEO_EDITOR_BENCH_WIDTH=320 VIDEO_EDITOR_BENCH_HEIGHT=180 \
VIDEO_EDITOR_BENCH_WARMUPS=0 VIDEO_EDITOR_BENCH_SAMPLES=1 \
cargo bench --bench animation_effects -- --nocapture
```

These are single-sample development measurements, not comparative release
benchmarks. All selected the CPU backend. Values are wall/frame-render ms/FPS:

| Scenario | Wall | Frame | FPS |
| --- | ---: | ---: | ---: |
| baseline | 1361 | 1130 | 105.80 |
| basic_colour | 1354 | 1134 | 106.35 |
| gaussian_small | 1875 | 1647 | 80.00 |
| gaussian_large | 2455 | 2241 | 61.10 |
| glow | 2081 | 1841 | 72.08 |
| sharpen | 1860 | 1626 | 80.65 |
| directional_blur | 3999 | 3761 | 37.51 |
| motion_blur | 3377 | 3149 | 44.42 |
| zoom_blur | 6384 | 6154 | 23.50 |
| blend_modes | 2442 | 2217 | 61.43 |
| global_post | 1875 | 1654 | 80.00 |
| impact | 1275 | 1042 | 117.65 |
| heavy_impact | 1408 | 1177 | 106.53 |
| transitions | 4560 | 4317 | 32.89 |
| combined | 8483 | 8266 | 21.22 |

A 720x1280 CPU baseline with the same one-sample, zero-warmup settings took
11,398 ms wall time, 10,882 ms frame rendering time, and 12.63 effective FPS.
The default five-warmup/five-sample 720x1280 command remains the release
measurement procedure; it was not run here because the full advanced matrix is
too expensive for this development environment.
