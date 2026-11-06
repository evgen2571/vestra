---
description: "Explore complete Vestra projects through their rendered output: motion graphics, photo cutouts, audio visualization and video editing."
---

# Examples

Browse complete Python projects by their output. Each page includes a preview
rendered with Vestra, the source code, and instructions for rendering it from
a checkout.

For a smaller introduction, start with the
[first render tutorial](../learn/first-render.md).

<div class="gallery-grid" markdown>

<div class="gallery-card" markdown>

[![Three cards animated against a dark background](../assets/gallery/motion-design.webp)](motion-design.md)

## [Animated layers and effects](motion-design.md)

A ten-second sequence combines moving shapes, masked video, image effects,
nested groups and audio-reactive graphics.

</div>

<div class="gallery-card" markdown>

[![One mountain photograph revealed through a circle and two rectangular cutouts](../assets/gallery/compositing.webp)](compositing.md)

## [Photo cutouts](compositing.md)

A landscape zooms inside three geometric cutouts. Each frame and its contents
move as a group, with a different saturation treatment.

</div>

<div class="gallery-card" markdown>

[![A mirrored peach-colored audio spectrum with a circular halo on a dark background](../assets/gallery/audio-reactive.webp)](audio-reactive.md)

## [Audio-reactive graphics](audio-reactive.md)

Frequency analysis controls the spectrum bars; loudness controls a halo and
particle opacity. The preview includes the synthesized soundtrack.

</div>

<div class="gallery-card" markdown>

[![Nighttime city footage with an overlaid Video editing title](../assets/gallery/video-editing.webp)](video-editing.md)

## [A three-shot video edit](video-editing.md)

Three city shots use slow zooms, half-second dissolves, colour adjustments and
a grouped title overlay.

</div>

</div>

## Run the examples

The commands below set up the repository examples. Unlike the standalone
tutorials, these projects load media and fonts from `examples/showcase/assets/`.

```bash
git clone https://github.com/evgen2571/vestra.git
cd vestra
git switch develop

# Enter the native development environment and build the Python package once.
nix develop
just python-sync
```

If you don't use Nix, follow the
[source-build installation instructions](../getting-started/installation.md#build-from-source--development)
for your platform. Building from the checkout needs Rust and native build tools;
rendering also needs `ffmpeg` and `ffprobe` on `PATH`.

Each project page gives its render command. Add `--smoke` for a 320 × 180,
6 fps version of the full timeline. Full renders use the CPU at 1280 × 720
and 30 fps; the website previews are downscaled copies.

The required media and fonts are committed with the examples, so rendering
needs no asset download after setup. Asset terms vary; see the
[preview provenance and credits](../assets/gallery/README.md) before sharing outputs.
