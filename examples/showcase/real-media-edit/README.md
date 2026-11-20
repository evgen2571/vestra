# Video editing

[Full video](render.mp4) · [Python source](main.py)

![Forest and valley footage with animated framing and dissolves](preview.gif)

Three shots use slow reframing, soft color grading, two dissolves and a grouped
title.

## Render

Run this from a [development checkout](../../../CONTRIBUTING.md#2-set-up-a-checkout).

```bash
uv run python examples/showcase/real-media-edit/main.py
```

Writes `examples/output/real-media-edit.mp4` (14 seconds, silent). Add
`--smoke` for a quick preview or `--output PATH` to choose the destination.

## Shared inputs

The edit reframes, grades and dissolves the first five seconds of the bundled
forest, valley and water-drop clips. Labels use Manrope.
[Footage credits](../../../assets/footage/README.md) · [Manrope font credits](../README.md#shared-font)

### Earlier footage copies

Older previews using “City timelapse video” must retain the following credit when shared.

> “City timelapse video” by Location Kenya, CC BY 3.0. Trimmed, resized,
> reframed, colour-adjusted and edited with the original audio removed.

Include the source and license links with the credit. No author endorsement is
implied. [Wikimedia Commons source](https://commons.wikimedia.org/wiki/File:City_timelapse_video.webm) · [CC BY 3.0 license](https://creativecommons.org/licenses/by/3.0/)
