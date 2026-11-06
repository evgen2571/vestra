# Video editing

[Watch / download the rendered video](render.mp4)

![Animated preview of the Vestra render](preview.gif)

A fourteen-second video editing example: three day-to-night Shanghai selections, slow reframing,
two dissolves, muted saturation/vignette, a grouped title and attribution. The output is silent. This demonstrates media → timeline → effects → output.

From the checkout after `uv sync --locked --extra dev`:

```bash
uv run python examples/showcase/real-media-edit/main.py
```

Output: `examples/output/real-media-edit.mp4`, 1280×720, 30 fps, 14 seconds, CPU.
Pass `--smoke` for the complete timeline at 320×180 and 6 fps, or `--output PATH`.
The three five-second selections are bundled, so the checkout
renders without a download. `prepare-assets.py` can reproduce them from the
verified original (about 18 MiB), which stays in an ignored local cache.

Footage is **CC0 1.0**, by Yigang Yu. Attribution is included as provenance. Read [ASSETS](../ASSETS.md)
for verified provenance, processing and redistribution requirements.
