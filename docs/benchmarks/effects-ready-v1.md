# Effects-ready v1 benchmark recipe

Run the existing benchmark harness with the CPU backend:

```bash
TMPDIR=target/tmp VIDEO_EDITOR_BENCH_BACKEND=cpu cargo bench --bench animation_effects
```

The harness reports 720x1280 resolution, frame count, selected backend,
median wall and render timing, frame composition time, encoding time, and
cache/decoded-resource peaks. Use the original color-only fixture as the
baseline. To measure an advanced chain, replace its clip effect array with the
ordered chain from `examples/projects/effects-ready-v1.json` and record the
same fields. CPU is the supported backend for advanced effects in v1.

Gaussian blur, glow, sharpen, and combined chains are expected to dominate
frame composition time. The renderer reuses two intermediate surfaces for the
ordered chain; it does not allocate a separate canvas for every pass.
