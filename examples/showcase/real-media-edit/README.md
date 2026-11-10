# Video editing

[Watch / download the rendered video](render.mp4)

![Animated preview of the Vestra render](preview.gif)

A fourteen-second video editing example: forest, valley and water-drop footage,
slow reframing, two dissolves, muted saturation/vignette and a grouped title.
The output is silent.

From the checkout after `uv sync --locked --extra dev`:

```bash
uv run python examples/showcase/real-media-edit/main.py
```

Output: `examples/output/real-media-edit.mp4`, 1280×720, 30 fps, 14 seconds, CPU.
Pass `--smoke` for the complete timeline at 320×180 and 6 fps, or `--output PATH`.
The three shots use the [shared bundled footage](../../../assets/footage/README.md),
also used by the benchmarks. Rendering needs no asset download.

## Shared inputs

The shots use the first five seconds of `forest.mp4`, `valley.mp4` and `tear.mp4`.
The [README demo](../readme-demo/README.md) uses the valley clip through a rotating
mask and dither effect. The edit reframes, grades and dissolves the same files.

Labels use Manrope; its [copyright and complete OFL license](../README.md#shared-font)
remain with the font. Videos display rendered text rather than distributing the
font file.

### Earlier footage copies

Earlier releases, branches, clones or shared previews may still contain “City
timelapse video” by Location Kenya, [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:City_timelapse_video.webm).
Those copies keep their license. When sharing them, retain: “City timelapse video —
Location Kenya, CC BY 3.0; trimmed, resized, reframed, colour-adjusted and edited
with the original audio removed.” Include the source and license links above;
no author endorsement is implied. Replacing this checkout's footage does not
relicense older copies.
