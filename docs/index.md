# Vestra documentation

Vestra is a native video composition and rendering engine with Python authoring
and a Rust rendering core. Start with [installation](getting-started/installation.md)
and the [Python quickstart](getting-started/python-quickstart.md), or the
[CLI quickstart](getting-started/cli-quickstart.md).

The [showcases](../examples/showcase/README.md) include finished videos, source
and assets. The [reference examples](../examples/README.md) focus on individual
features. [Project schema v1](../schemas/project.schema.json) is the canonical
machine-readable contract.

## Getting started

- [CLI quickstart](getting-started/cli-quickstart.md)
- [Install Vestra](getting-started/installation.md)
- [Python quickstart](getting-started/python-quickstart.md)

## Concepts

- [Audio and signals](concepts/audio-and-signals.md)
- [The authoring model](concepts/authoring-model.md)
- [Effects, transitions, animation, flashes, and presets](concepts/effects-transitions-and-animation.md)
- [Rendering backends](concepts/rendering-backends.md)
- [The rendering lifecycle](concepts/rendering-lifecycle.md)
- [Timeline and time](concepts/timeline-and-time.md)

## Python guides

- [Animate properties](guides/python/animation.md)
- [Add audio](guides/python/audio.md)
- [Add effects](guides/python/effects.md)
- [Layer masks](guides/python/masks.md)
- [Use nested compositions](guides/python/nested-compositions.md)
- [Projects and compositions](guides/python/projects-and-compositions.md)
- [Render and prepare from Python](guides/python/rendering-and-preparation.md)
- [Signals and audio reactivity](guides/python/signals-and-audio-reactivity.md)
- [Sources and layers](guides/python/sources-and-layers.md)
- [Transitions, flashes, and presets](guides/python/transitions-flashes-and-presets.md)

## CLI guides

- [Logging and progress](guides/cli/logging-and-progress.md)
- [Render with the CLI](guides/cli/rendering.md)

## Reference

- [Audio](reference/audio.md)
- [Backends](reference/backends.md)
- [CLI](reference/cli.md)
- [Diagnostics](reference/diagnostics.md)
- [Effects](reference/effects.md)
- [Environment variables](reference/environment-variables.md)
- [Feature support](reference/feature-support.md)
- [Masks reference](reference/masks.md)
- [Nested compositions](reference/nested-compositions.md)
- [Presets and flashes](reference/presets-and-flashes.md)
- [Project format](reference/project-format.md)
- [Python API](reference/python-api.md)
- [Rust SDK](reference/rust-sdk.md)
- [Signals](reference/signals.md)
- [Transitions](reference/transitions.md)

## Source reference

- [Image source](reference/sources/image.md)
- [Particle system source](reference/sources/particle-system.md)
- [Shape source](reference/sources/shape.md)
- [Solid-color source](reference/sources/solid-color.md)
- [Spectrum2D source](reference/sources/spectrum2d.md)
- [Text source](reference/sources/text.md)
- [Video source](reference/sources/video.md)

## Troubleshooting

- [FFmpeg and media problems](troubleshooting/ffmpeg.md)
- [Python problems](troubleshooting/python.md)
- [Rendering problems](troubleshooting/rendering.md)
- [WGPU problems](troubleshooting/wgpu.md)

## Contributor procedures

- [Contributing](development/contributing.md)
- [CPU rendering and work reuse](development/cpu-efficiency.md)
- [GPU validation](development/gpu-validation.md)
- [Media pipeline and concurrency](development/media-pipeline.md)
- [Performance](development/performance.md)
- [v0.1.0 release notes draft](development/release-notes.md)
- [Releasing the Python package](development/releasing.md)
- [Testing](development/testing.md)

## Architecture

- [CPU renderer](development/architecture/cpu-renderer.md)
- [Crate ownership](development/architecture/crates.md)
- [Media I/O](development/architecture/media-io.md)
- [Architecture overview](development/architecture/overview.md)
- [Project to plan](development/architecture/project-to-plan.md)
- [Python binding architecture](development/architecture/python-bindings.md)
- [Render pipeline](development/architecture/render-pipeline.md)
- [WGPU renderer](development/architecture/wgpu-renderer.md)

## Extension guides

- [Add a backend](development/extending/backend.md)
- [Add an effect](development/extending/effect.md)
- [Add a source](development/extending/source.md)
- [Add a transition](development/extending/transition.md)
