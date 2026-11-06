---
description: Assemble three city shots with animated reframing, overlapping dissolves, colour effects and a grouped title in Vestra.
---

# A three-shot video edit

This example combines three five-second city shots into a fourteen-second
video. Each shot slowly zooms in; half-second overlaps provide the dissolves.
A title appears over the opening shot, and a footage credit appears at the end.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/video-editing.webp" aria-label="Fourteen-second silent edit of city timelapse footage">
    <source src="../assets/gallery/video-editing.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/video-editing.mp4">Download the preview</a>.
  </video>
  <figcaption>Three night-city views slowly reframe and dissolve into each other. A title fades in above a thin rule, and the final shot fades out with a footage credit. The preview is silent.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/real-media-edit/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/real-media-edit/render.mp4)

## Reproduce the example

After the [one-time gallery setup](index.md#run-the-examples), run:

```bash
uv run --no-sync python examples/showcase/real-media-edit/main.py
```

Open `examples/output/real-media-edit.mp4`: fourteen seconds, 1280 × 720, 30 fps,
with no audio. The script replaces this output on each run.
Use `--output PATH` for a different destination or `--smoke` for the full timeline
at a smaller size and lower frame rate.

Three five-second footage selections and the Manrope font are included in the
checkout. You do not need to download the larger original video to render this
project. To reproduce the bundled input selections, use the
[asset preparation instructions](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/ASSETS.md);
that optional step downloads a checksum-verified original.

## Create a dissolve with overlapping layers

The clips start at 0, 4.5 and 9 seconds and each lasts five seconds. Their
half-second overlaps give the incoming shot something to fade over. The incoming
shot's opacity rises from zero to one while the outgoing shot stays opaque;
fading both at once would expose the background during the dissolve.

The title and its thin rule live in one group, so a single opacity track fades
the overlay together. Saturation and vignette are applied to each footage layer.

See [Sources and layers](../guides/python/sources-and-layers.md),
[Animation and keyframes](../learn/animation.md), and
[Rendering and preparation](../guides/python/rendering-and-preparation.md)
to adapt the sequence to your own footage.

## Credits

“City timelapse video” by Location Kenya,
[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:City_timelapse_video.webm).
Trimmed, resized, reframed, colour-adjusted and edited with the original audio
removed. The documentation preview is additionally downscaled and compressed.
No author endorsement is implied.

Keep this credit and the source/license links with redistributed outputs.
The Manrope font is SIL OFL 1.1; full input provenance is in the
[preview credits](../assets/gallery/README.md).

[Back to the gallery](index.md)
