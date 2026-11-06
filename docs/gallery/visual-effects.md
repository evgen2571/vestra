---
description: Compare a landscape photograph with halftone and ASCII treatments using the same zoom animation and rounded masks.
---

# Halftone and ASCII effects

Display the same photograph in three panels to compare the original with two
effects. One panel uses warm halftone dots; another uses green ASCII characters.
All three follow the same eased zoom, making the differences easy to see.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/visual-effects.webp" aria-label="Six-second silent comparison of a landscape photograph, halftone dots and ASCII characters">
    <source src="../assets/gallery/visual-effects.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/visual-effects.mp4">Download the preview</a>.
  </video>
  <figcaption>The photograph slowly zooms in and out across all three panels. Halftone converts image detail into dots; ASCII converts it into a grid of characters. The preview is silent.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/lookbook/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/lookbook/render.mp4)

## Reproduce the example

After the [one-time gallery setup](index.md#run-the-examples), run:

```bash
uv run --no-sync python examples/showcase/lookbook/main.py
```

Open `examples/output/lookbook.mp4`: six seconds, 1280 × 720, 30 fps, with no
audio. The script replaces this output when rerun. Use `--output PATH` for a
different destination or `--smoke` for a smaller version of the full timeline.

The landscape photograph and Manrope font are bundled with the examples. The
ASCII effect uses Vestra's bundled DejaVu face; no system font installation is
needed.

## Compare treatments on the same source

Each panel contains the same image with the same scale keyframes. The first
has no effect. The second attaches `Halftone`; the third attaches `Ascii`.
The effects' cell sizes control how much detail each dot or character covers,
while foreground and background colours define the treatment's palette.

A rounded rectangle mask belongs to each panel's group, so the crop stays the
same while the image zooms inside it. Labels remain outside these masked groups.
All effects, masks, animation and text are rendered by Vestra.

See [Visual effects](../guides/python/effects.md),
[Effect parameters](../reference/effect-parameters.md), and
[Masks](../guides/python/masks.md) for the supported controls.

## Credits

“Landscape in Salzburger Land” by Dimitry Anikin,
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Landscape_in_Salzburger_Land.jpg).
The photograph is resized, cropped, animated and processed with halftone and
ASCII effects.

Manrope is SIL OFL 1.1. The bundled DejaVu face retains its Bitstream Vera/DejaVu
notice. See the [preview credits](../assets/gallery/README.md) for both font
licenses and original source links.

[Back to the gallery](index.md)
