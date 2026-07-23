# Effects-ready v1 benchmark recipe

Run the harness with the CPU backend. It defaults to the basic-colour scene:

```bash
TMPDIR=target/tmp VIDEO_EDITOR_BENCH_BACKEND=cpu cargo bench --bench animation_effects
```

Select an advanced scenario with `VIDEO_EDITOR_BENCH_SCENARIO`. The available
scenarios are `baseline`, `basic_colour`, `gaussian_small`, `gaussian_large`,
`glow`, `sharpen`, `directional_blur`, `motion_blur`, `blend_modes`,
`global_post`, `impact`, `heavy_impact`, `transitions`, and `combined`.

```bash
TMPDIR=target/tmp VIDEO_EDITOR_BENCH_BACKEND=cpu \
  VIDEO_EDITOR_BENCH_SCENARIO=combined cargo bench --bench animation_effects
```

The harness reports 720x1280 resolution, frame count, selected backend,
median wall and render timing, frame composition time, encoding time, and
cache/decoded-resource peaks. Use the original color-only fixture as the
baseline. To measure an advanced chain, replace its clip effect array with the
ordered chain from `examples/projects/effects-ready-v1.json` and record the
same fields. CPU is the supported backend for advanced effects in v1.

Gaussian blur, glow, sharpen, and combined chains are expected to dominate
frame composition time. Record the median wall time, frame-render time, and
effective FPS for each scenario alongside the baseline. The renderer reuses
its intermediate surfaces for ordered chains; it does not allocate one canvas
per pass.
