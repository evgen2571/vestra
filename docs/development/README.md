# Vestra development

Start with the root [contribution guide](../../CONTRIBUTING.md) for setup and
pull request guidance. Use [Testing](testing.md) to choose checks for your change,
and the [architecture overview](architecture/overview.md) to find your way
around the codebase. Agents should also consult [AGENTS.md](../../AGENTS.md),
[PLANS.md](../../PLANS.md) and relevant [active plans](../plans/active/stylized-video-effects.md).

## Testing and performance

- [Testing](testing.md)
- [GPU validation](gpu-validation.md)
- [Performance](performance.md)
- [CPU rendering and work reuse](cpu-efficiency.md)
- [Media pipeline and concurrency](media-pipeline.md)

## Architecture

- [Architecture overview](architecture/overview.md)
- [Effect pipeline and resource constraints](architecture/effect-pipeline.md)
- [Crate ownership](architecture/crates.md)
- [Project to plan](architecture/project-to-plan.md)
- [Render pipeline](architecture/render-pipeline.md)
- [CPU renderer](architecture/cpu-renderer.md)
- [WGPU renderer](architecture/wgpu-renderer.md)
- [Media I/O](architecture/media-io.md)
- [Python binding architecture](architecture/python-bindings.md)

## Stylization references

- [Detailed, palette-independent dithered look](stylization/dithered-palette-look.md)

## Extension guides

- [Add a source](extending/source.md)
- [Add an effect](extending/effect.md)
- [Add a transition](extending/transition.md)
- [Add a backend](extending/backend.md)

## Release procedures

- [Releasing the Python package](releasing.md)
- [v0.1.0 historical release summary](release-notes.md)
