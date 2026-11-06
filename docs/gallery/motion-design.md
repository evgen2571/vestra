---
description: Three scenes demonstrating grouped animation, masked Shanghai footage, ordered dithering and audio-reactive graphics.
---

# Layers, masks and audio signals

This ten-second composition contains three scenes: numbered cards move as
nested groups, Shanghai footage plays inside a rotating mask, and a radial
spectrum responds to a synthesized soundtrack. The scene captions identify the
features as they appear.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/motion-design.webp" aria-label="Ten-second silent composition showing animated cards, masked Shanghai footage and an audio-reactive radial spectrum">
    <source src="../assets/gallery/motion-design.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/motion-design.mp4">Download the preview</a>.
  </video>
  <figcaption>Cards and their inset labels move together. Blue-hour footage is revealed through a rounded mask, then changes to a four-colour dithered image. A spectrum and pulsing ring finish the sequence. The preview is silent.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/readme-demo/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/readme-demo/render.mp4)

## Reproduce the example

After the [one-time gallery setup](index.md#run-the-examples), run this from the
repository root:

```bash
uv run --no-sync python examples/showcase/readme-demo/main.py
```

Open `examples/output/readme-demo.mp4`: ten seconds, 1280 × 720, 30 fps.
The script replaces that output when rerun. To choose another destination:

```bash
uv run --no-sync python examples/showcase/readme-demo/main.py \
  --output examples/output/animated-scenes.mp4
```

The project uses the bundled Manrope font, blue-hour Shanghai selection and
synthesized soundtrack. `output_audio=False` excludes audio from the exported
video while keeping it available for analysis.

## Organize related layers into scenes

The local `scene()` helper creates a group with a start time, duration and
opacity animation. Each scene fades in and out over a persistent background.
Moving a scene later in the composition preserves the timing of its children.

The card scene adds another level of groups: each card contains a rectangle,
a circle and a label. Animating the card's transform moves all three together.
In the media scene, `OrderedDither.amount` changes from zero to one, introducing
the palette treatment after the footage is visible. The final scene combines
`Spectrum2D` frequency analysis with an RMS signal bound to the ring's scale.

For these techniques, see
[Animation and keyframes](../learn/animation.md),
[Groups and nested compositions](../guides/python/nested-compositions.md),
[Masks](../guides/python/masks.md), and
[Audio-reactive authoring](../guides/python/signals-and-audio-reactivity.md).

## Credits

“Shanghai Bund, Lujiazui Skyline Timelapse” by Yigang Yu,
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Shanghai_Bund,_Lujiazui_Skyline_Timelapse.webm).
The footage is trimmed, resized, reframed, masked and dithered; its original
audio is removed. The documentation preview is additionally downscaled and
compressed. Attribution is retained as provenance.

The original synth loop is CC0; Manrope is SIL OFL 1.1. Its [example credits](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/readme-demo/README.md#credits) link to
the original synth dedication, footage preparation and accompanying font license.

## Preview details

This is a downscaled copy of the complete CPU render: 640 × 360, 30 fps,
H.264/yuv420p with MP4 fast start. The WebP poster is the frame at 1.7 seconds.
The original render and cleaned source are together at the
[reviewed example revision](https://github.com/evgen2571/vestra/blob/84c042123474eaa5176585d72d5605b043a5a251/examples/showcase/readme-demo/README.md).
The source render's SHA-256 is `38b6d16c31f5580578029ff5f4613a20e10a7883965bcadec9231892bf9e752e`.

[Reproduce the delivery copies](../development/documentation.md#reproduce-gallery-previews).
The previews contain rendered text; they do not distribute font files.

[Back to the gallery](index.md)
