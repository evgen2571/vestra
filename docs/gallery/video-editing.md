---
description: Combine daytime, blue-hour and nighttime Shanghai footage with animated reframing, dissolves, colour effects and a grouped title.
---

# A day-to-night video edit

This example combines three five-second Shanghai shots into a fourteen-second
video, moving from daylight through blue hour to night. Each shot slowly zooms
in; half-second overlaps provide the dissolves. A title appears over the
opening shot, and a footage credit appears at the end.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/video-editing.webp" aria-label="Fourteen-second silent edit of Shanghai skyline footage from day through blue hour to night">
    <source src="../assets/gallery/video-editing.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/video-editing.mp4">Download the preview</a>.
  </video>
  <figcaption>Daytime, blue-hour and night views of the Shanghai skyline dissolve into each other with slow reframing. The title and its rule fade together, and the final shot fades out with a footage credit. The preview is silent.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/real-media-edit/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/real-media-edit/render.mp4)

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
checkout. To reproduce the selections from the larger original video, use the
[asset preparation instructions](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/README.md#prepare-shared-inputs).
That optional step downloads a checksum-verified 18 MiB original.

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

“Shanghai Bund, Lujiazui Skyline Timelapse” by Yigang Yu,
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Shanghai_Bund,_Lujiazui_Skyline_Timelapse.webm).
Trimmed, resized, reframed, colour-adjusted and edited with the original audio
removed. The documentation preview is additionally downscaled and compressed.
Attribution is retained as provenance; CC0 does not require it.

The Manrope font is SIL OFL 1.1. Its [example README](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/real-media-edit/README.md#footage-source)
records original source links, preparation, checksum and the accompanying font notice.
[Earlier Location Kenya copies](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/real-media-edit/README.md#earlier-footage-copies)
retain their CC BY 3.0 attribution requirements.

## Preview details

This is a downscaled copy of the complete CPU render: 640 × 360, 30 fps,
H.264/yuv420p with MP4 fast start. The WebP poster is the frame at 6 seconds.
The original render and cleaned source are together at the
[reviewed example revision](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/real-media-edit/README.md).
The source render's SHA-256 is `16ac337002626f69e6797f9291ad6b3bea5967f4da32a81451a2a0ace52ff933`.

[Reproduce the delivery copies](../development/documentation.md#reproduce-gallery-previews).
The previews contain rendered text; they do not distribute font files.

[Back to the gallery](index.md)
