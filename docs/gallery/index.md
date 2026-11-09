---
description: "Complete Vestra Python projects with previews: animated layers, masked images, audio-reactive graphics, video editing and visual effects."
---

# Examples

Browse complete Python projects by their output. Each page includes a preview
rendered with Vestra, the source code, and instructions for rendering it from
a checkout.

For a smaller introduction, start with the
[first render tutorial](../learn/first-render.md).

<div class="gallery-grid" markdown>

<div class="gallery-card" markdown>

[![Overlapping numbered cards beside a Video composition heading](../assets/gallery/motion-design.webp)](motion-design.md)

## [Layers, masks and audio signals](motion-design.md)

Three scenes show grouped card animation, masked Shanghai footage with ordered
dithering, and an audio-reactive radial spectrum.

</div>

<div class="gallery-card" markdown>

[![One mountain photograph revealed through a circle and two rectangular cutouts](../assets/gallery/compositing.webp)](compositing.md)

## [Photo cutouts](compositing.md)

A landscape zooms inside three geometric cutouts. Each frame and its contents
move as a group, with a different saturation treatment.

</div>

<div class="gallery-card" markdown>

[![Cyan and coral spectrum bars arranged around a ring on a dark purple background](../assets/gallery/audio-reactive.webp)](audio-reactive.md)

## [Audio-reactive graphics](audio-reactive.md)

Frequency analysis controls 48 radial spectrum bars; loudness controls a ring
and particle opacity. The preview includes the synthesized soundtrack.

</div>

<div class="gallery-card" markdown>

[![Shanghai skyline at blue hour with illuminated buildings reflected in the river](../assets/gallery/video-editing.webp)](video-editing.md)

## [A day-to-night video edit](video-editing.md)

Three Shanghai shots use slow zooms, half-second dissolves, colour adjustments
and a grouped title overlay.

</div>

<div class="gallery-card" markdown>

[![A landscape shown side by side as a photograph, warm halftone dots and green ASCII characters](../assets/gallery/visual-effects.webp)](visual-effects.md)

## [Halftone and ASCII effects](visual-effects.md)

Compare two effects with the original photograph. The three masked panels
share the same zoom animation.

</div>

</div>

## Run the examples

These projects load media and fonts from `examples/showcase/assets/`. The
sources and inputs are included in the documentation checkout. If you already
have that checkout, enter `nix develop` and run `just python-sync` from its root.

To obtain the exact example versions used for these previews independently:

```bash
git clone https://github.com/evgen2571/vestra.git
cd vestra

# Keep the example sources and input assets at the captured revision.
git switch --detach fa0e95a1c896d01798f54e27d7e9bed3623734c7

# Enter the native development environment and build the Python package once.
nix develop
just python-sync
```

Source links, bundled inputs and full-resolution renders all use revision
`fa0e95a1c896d01798f54e27d7e9bed3623734c7`. Its source cleanup preserves the scenes used for the existing previews;
no media was regenerated for naming, layout or comment changes. A review checkout
of this branch already contains that revision. Public commit links become
available when the reviewed commits are pushed.

Each example's credits describe its inputs and preview. To make website delivery
copies from the bundled renders, follow [preview reproduction](../development/documentation.md#reproduce-gallery-previews);
it uses this checkout and does not fetch a separate documentation branch.

If you don't use Nix, follow the
[source-build installation instructions](../getting-started/installation.md#build-from-source--development)
for your platform. Building from the checkout needs Rust and native build tools;
developer media tools also need `ffmpeg` on `PATH`. Current Python installs its
FFmpeg executable automatically. The captured revision
above also uses `ffprobe` for audio probing. Current Vestra probes media through
bundled libraries and does not require that executable.

Each project page gives its render command. Add `--smoke` for a 320 × 180,
6 fps version of the full timeline. Full renders use the CPU at 1280 × 720
and 30 fps with Vestra's high quality tier; the website previews are downscaled
copies.

The required media and fonts are committed with the examples, so rendering
needs no asset download after setup. Source, license and modification details
are beside each example under **Credits**, with reproduction details in its
linked README.
