---
description: Combine forest, valley and water-drop footage with animated reframing, dissolves, colour effects and a grouped title.
---

# A three-shot video edit

This example combines forest, valley and water-drop footage into a fourteen-second
video. Each shot slowly zooms in; half-second overlaps provide the dissolves.
A grouped title appears over the opening shot, and the final shot fades out.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/video-editing.webp" aria-label="Fourteen-second silent edit of forest, valley and water-drop footage">
    <source src="../assets/gallery/video-editing.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/video-editing.mp4">Download the preview</a>.
  </video>
  <figcaption>Three shots dissolve into each other with slow reframing. The title and its rule fade together, and the final shot fades out. The preview is silent.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/f4679ecdc4d7a45b6b56e5d106f873d3374492dd/examples/showcase/real-media-edit/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/f4679ecdc4d7a45b6b56e5d106f873d3374492dd/examples/showcase/real-media-edit/render.mp4)

## Reproduce the example

After the [one-time gallery setup](index.md#run-the-examples), run:

```bash
uv run --no-sync python examples/showcase/real-media-edit/main.py
```

Open `examples/output/real-media-edit.mp4`: fourteen seconds, 1280 × 720, 30 fps,
with no audio. The script replaces this output on each run.
Use `--output PATH` for a different destination or `--smoke` for the full timeline
at a smaller size and lower frame rate.

The footage and Manrope font are included in the checkout. These are the same
video inputs used by the [benchmarks](../benchmarks/workloads.md#shared-footage).
Rendering needs no download after setup.

## Create a dissolve with overlapping layers

The clips start at 0, 4.5 and 9 seconds and each lasts five seconds. Their
half-second overlaps give the incoming shot something to fade over. The incoming
shot's opacity rises from zero to one while the outgoing shot stays opaque;
fading both at once would expose the background during the dissolve.

The title and its thin rule live in one group, so a single opacity track fades
the overlay together. Saturation and vignette are applied to each footage layer.

See [Sources and layers](../guides/python/sources-and-layers.md),
[Animation and keyframes](../getting-started/animation.md), and
[Rendering and preparation](../guides/python/rendering-and-preparation.md)
to adapt the sequence to your own footage.

## Credits

The Manrope font is SIL OFL 1.1; the [shared font notice](https://github.com/evgen2571/vestra/blob/f4679ecdc4d7a45b6b56e5d106f873d3374492dd/examples/showcase/README.md#shared-font)
includes its copyright and complete license.
[Earlier Location Kenya copies](https://github.com/evgen2571/vestra/blob/f4679ecdc4d7a45b6b56e5d106f873d3374492dd/examples/showcase/real-media-edit/README.md#earlier-footage-copies)
retain their CC BY 3.0 attribution requirements.

## Preview details

This is a downscaled copy of the complete CPU render: 640 × 360, 30 fps,
H.264/yuv420p with MP4 fast start. The WebP poster is the frame at 6 seconds.
The original render and cleaned source are together at the
[reviewed example revision](https://github.com/evgen2571/vestra/blob/f4679ecdc4d7a45b6b56e5d106f873d3374492dd/examples/showcase/real-media-edit/README.md).
The source render's SHA-256 is `091e10cf2d19a84ddf0c9c0fabe12d83764606322272d94860ce44c1889d61b9`.

[Reproduce the delivery copies](../development/documentation.md#reproduce-gallery-previews).
The previews contain rendered text; they do not distribute font files.

[Back to the gallery](index.md)
