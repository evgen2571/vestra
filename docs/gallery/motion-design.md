---
description: A ten-second sequence demonstrating layer animation, masked video, effects, nested groups and audio-reactive graphics.
---

# Animated layers and effects

This project arranges five scenes in a ten-second composition. Animated cards
are followed by masked city footage, a photograph with effects, nested groups,
and an audio-reactive scene. A shared scene builder supplies the background
and title; each scene defines its own layers and animation.

<figure>
  <video controls preload="metadata" playsinline width="640" height="360" poster="../../assets/gallery/motion-design.webp" aria-label="Ten-second silent sequence demonstrating animated layers, masks, effects and audio-reactive graphics">
    <source src="../assets/gallery/motion-design.mp4" type="video/mp4">
    Your browser cannot play this video. <a href="../assets/gallery/motion-design.mp4">Download the preview</a>.
  </video>
  <figcaption>Cards slide and rotate into place; a soft mask reveals city footage; a photograph resolves into focus; grouped cards lead into a halo, particles and an audio spectrum. The preview is silent.</figcaption>
</figure>

[Complete Python source](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/readme-demo/main.py){ .md-button }
[Full-resolution render](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/readme-demo/render.mp4)

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

The project uses the bundled Manrope font, landscape photograph, city selections
and synthesized soundtrack. The soundtrack drives analysis but is deliberately
excluded from this video's output.

## How the scenes are organized

`build_layers_scene()` assigns a position, scale and rotation track to each
card. `create_scene()` places the content of each scene inside a group, so the
entire scene can enter or leave as one unit.

Each group has its own start and duration. Its children use scene-local times,
so moving a scene later in the composition preserves the timing of the
animations inside it.

For the individual techniques, see
[Animation and keyframes](../learn/animation.md),
[Groups and nested compositions](../guides/python/nested-compositions.md), and
[Visual effects](../guides/python/effects.md). The
[first animation tutorial](../learn/first-animation.md) introduces the same
keyframe API in a smaller project.

## Credits

“City timelapse video” by Location Kenya,
[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:City_timelapse_video.webm).
Trimmed, resized, reframed, colour-adjusted and edited with the original audio
removed. The documentation preview is additionally downscaled and compressed.
No author endorsement is implied.

The photograph by Dimitry Anikin is CC0; Manrope is SIL OFL 1.1. Full asset
provenance and license links are in the
[preview credits](../assets/gallery/README.md).

[Back to the gallery](index.md)
