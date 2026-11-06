---
description: Reveal a single photograph through three geometric masks, then animate each nested composition independently with Vestra.
---

# Photo cutouts

A mask decides which part of a layer is visible. Here, one landscape appears in
three different frames while the photograph slowly zooms underneath each one.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/compositing.webp" aria-label="Eight-second silent photo cutout animation">
    <source src="../assets/gallery/compositing.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/compositing.mp4">Download the preview</a>.
  </video>
  <figcaption>A circle, a rounded rectangle and a rectangle rise into view and drift gently. The image in each cutout zooms independently. The left cutout is monochrome; the other two use different saturation settings. The cutouts fade out together at the end.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/compositing/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/compositing/render.mp4)

## Reproduce the example

After the [one-time gallery setup](index.md#run-the-examples), run:

```bash
uv run --no-sync python examples/showcase/compositing/main.py
```

Open `examples/output/compositing.mp4`: eight seconds, 1280 × 720, 30 fps,
with no audio. The script replaces this output on each run.
Use `--output PATH` to choose another file or `--smoke` to render a smaller,
lower-frame-rate version of the complete timeline.

The Manrope font and landscape photograph are bundled. No download or additional
Python package is needed beyond the gallery setup.

## Animate the mask and image separately

In `build_project()`, each `card` is a group. The photo belongs to the group, and
the geometric mask is attached to that group. Moving the card moves both the
photo and its frame. Changing the photo's scale zooms the contents while the
mask stays the same size.

To compare another mask shape, replace the circle in the first group with a
rounded rectangle. To compare colour treatments, change one saturation value
to `0` for monochrome. Neither change affects the other two groups.

Continue with [Masks](../guides/python/masks.md),
[Groups and nested compositions](../guides/python/nested-compositions.md), and
[Image sources](../reference/sources/image.md).

## Credits

“Landscape in Salzburger Land” by Dimitry Anikin,
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Landscape_in_Salzburger_Land.jpg).
The image was resized, cropped, zoomed, masked and colour-adjusted.
The Manrope font is SIL OFL 1.1. The [example README](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/compositing/README.md#photo-source)
records the photo checksum, resizing command and accompanying font notice.

## Preview details

This is a downscaled copy of the complete CPU render: 640 × 360, 30 fps,
H.264/yuv420p with MP4 fast start. The WebP poster is the frame at 2 seconds.
The original render and cleaned source are together at the
[reviewed example revision](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/compositing/README.md).
The source render's SHA-256 is `382c4376de3ebfd5dc68cbe40239b00402da8a05d6db1ee478dc3642e5c2903b`.

[Reproduce the delivery copies](../development/documentation.md#reproduce-gallery-previews).
The previews contain rendered text; they do not distribute font files.

[Back to the gallery](index.md)
