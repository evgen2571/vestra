---
title: Vestra documentation
description: Documentation for Vestra, a Python video composition library backed by Rust. Installation, tutorials, rendered examples, and API reference.
hide:
  - navigation
  - toc
---

<div class="vestra-home" markdown="1">

<div class="home-hero" markdown="0">
<div class="home-hero__copy">
<h1>Video composition<br> in Python</h1>
<p class="home-lead">Vestra is a Python library for composing and rendering video, backed by Rust. Combine footage, text, shapes, effects, and audio in a script.</p>
<div class="home-actions">
<a class="home-button home-button--primary" href="getting-started/installation/">Install Vestra</a>
<a class="home-button" href="gallery/">Browse examples</a>
</div>
<p class="home-requirements">Python 3.11–3.14 · CPU or WGPU rendering · MP4 output</p>
</div>
<figure class="home-hero__preview">
<video autoplay muted loop controls playsinline preload="metadata" width="640" height="360" poster="assets/hero/poster.webp" aria-label="Vestra demo" aria-describedby="reel-caption">
<source src="assets/hero/reel.mp4" type="video/mp4">
</video>
<figcaption id="reel-caption">A tile animation in Python, followed by compositing, audio-reactive motion, and video editing examples. <a href="assets/hero/reel.mp4" download>Download MP4</a> · <a href="https://github.com/evgen2571/vestra/blob/develop/examples/showcase/readme-hero/README.md">Source &amp; credits</a></figcaption>
</figure>
</div>

<div class="home-intro" markdown="1">

## Projects, layers, and rendering

A project defines the canvas and timeline. Add sources as layers, set their properties, and use keyframes to change those properties over time. Vestra evaluates the composition and encodes the result as an MP4.

Because the composition is Python code, you can use data, loops, and functions to produce related videos from the same script. The examples below cover motion graphics, masked images, video editing, and audio-reactive graphics.

</div>

<div class="home-code" markdown="1">
<div markdown="1">

### A complete rendering example

Three position keyframes move this rounded rectangle across the canvas and back. It renders on CPU and writes `quickstart.mp4` to the current directory; rerunning it replaces that file.

```python
--8<-- "examples/tutorials/first_animation.py"
```

Save the example as `first_animation.py` after
[installing Vestra](getting-started/installation.md), then run:

```bash
uv run python first_animation.py
```

This creates `quickstart.mp4` in the current directory and replaces that file
on a rerun. Follow the [first render tutorial](getting-started/first-render.md) for a complete, explained project.

</div>
<figure>
<video controls playsinline preload="metadata" width="640" height="360" poster="assets/renders/first-animation.webp" aria-label="A mint rounded rectangle moves right and returns over three seconds">
<source src="assets/renders/first-animation.mp4" type="video/mp4">
<a href="assets/renders/first-animation.mp4">Download the rectangle animation</a>.
</video>
<figcaption>The output of the code alongside it. Positions use fractions of the canvas: (0.5, 0.5) is the center.</figcaption>
</figure>
</div>

## Documentation version

This site follows the **`develop` branch**, which can include changes after the
latest [published release](https://github.com/evgen2571/vestra/releases).
Tutorial downloads come from that branch too. Compare your installed version
with the release notes when an API differs; for a development checkout, use
[Build from source](development/source-builds.md). Gallery pages identify the
exact source revision used for their recorded previews.

## Learn Vestra

The introductory tutorials start with the rectangle's static layout, then add the position track from the example above. Each includes complete source and the expected output.

<ol class="learning-path">
<li>
<span class="learning-path__step" aria-hidden="true">1</span>
<h3><a href="getting-started/installation/">Install Vestra</a></h3>
<p>Install the Python package, then check your environment.</p>
</li>
<li>
<span class="learning-path__step" aria-hidden="true">2</span>
<h3><a href="getting-started/first-render/">Render a shape</a></h3>
<p>Define a canvas, place a rounded rectangle, and render a three-second MP4.</p>
</li>
<li>
<span class="learning-path__step" aria-hidden="true">3</span>
<h3><a href="getting-started/first-animation/">Animate the layers</a></h3>
<p>Add eased position keyframes to move the rectangle right and back.</p>
</li>
</ol>

[Animation and keyframes](getting-started/animation.md) explains interpolation, easing, and layer-local time in more detail.

## Rendered examples

<div class="home-gallery" markdown="0">
<a href="gallery/visual-effects/" class="home-gallery__item"><img src="assets/gallery/visual-effects.webp" width="640" height="360" loading="lazy" alt="A landscape displayed as a photograph, halftone, and ASCII"><span><strong>Photo effects</strong><span>Halftone, ASCII, and group masks</span></span></a>
<a href="gallery/audio-reactive/" class="home-gallery__item"><img src="assets/gallery/audio-reactive.webp" width="640" height="360" loading="lazy" alt="A cyan and coral radial spectrum surrounding a pulsing ring"><span><strong>Audio-reactive graphics</strong><span>Audio analysis, signals, and particles</span></span></a>
<a href="gallery/video-editing/" class="home-gallery__item"><img src="assets/gallery/video-editing.webp" width="640" height="360" loading="lazy" alt="Snowy valley footage with animated reframing"><span><strong>Video editing</strong><span>Video layers, reframing, and transitions</span></span></a>
</div>

[Browse all examples and their source code](gallery/index.md), or see the same bundled footage in the [benchmark workloads](benchmarks/workloads.md).

<div class="home-reference" markdown="1">

## Guides and API reference

Use the **Guides** navigation for tasks such as [grouping layers](guides/python/nested-compositions.md), [masking images](guides/python/masks.md), [applying effects](guides/python/effects.md), or [working with audio and spectra](guides/python/signals-and-audio-reactivity.md). **Concepts** explains the authoring and timing model. **Reference** lists [source types](reference/index.md#choose-a-source), APIs, parameters and support limits.

Vestra renders finished videos to H.264 MP4. The Python API is evolving in the 0.1 series. Check [feature support](reference/feature-support.md) for backend coverage and [troubleshooting](troubleshooting/rendering.md) if a render fails.

</div>
</div>
