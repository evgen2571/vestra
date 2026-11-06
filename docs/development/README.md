<span id="vestra-development"></span>

# Contributing to Vestra

Start with the root [contribution guide](https://github.com/evgen2571/vestra/blob/develop/CONTRIBUTING.md) for setup and
pull request guidance. Use [Testing](testing.md) to choose checks for your change,
and the [architecture overview](architecture/overview.md) to find your way
around the codebase. For site content and previews, use
[Documentation authoring](documentation.md). Agents should also consult [AGENTS.md](https://github.com/evgen2571/vestra/blob/develop/AGENTS.md)
and [PLANS.md](https://github.com/evgen2571/vestra/blob/develop/PLANS.md).

The [Vestra skill index](https://github.com/evgen2571/vestra/blob/develop/.agents/skills/README.md) routes agent work to
task-specific verification. GitHub [issue workflow](https://github.com/evgen2571/vestra/blob/develop/docs/agents/issue-tracker.md)
owns specs and dependencies; implementation plans track work on a selected issue.

## Testing and performance

- [Testing](testing.md)
- [GPU validation](gpu-validation.md)
- [Performance and benchmarking](performance.md)
- [Historical Vestra/MoviePy benchmark investigation](../benchmarks/vestra-vs-moviepy.md)
- [Media pipeline and concurrency](media-pipeline.md)
- [Timeline domains and validation](architecture/project-to-plan.md#time-domains-and-frame-boundaries)

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
- [Stylization temporal diagnostics](stylization/temporal-diagnostics.md)

## Extension guides

- [Add a source](extending/source.md)
- [Add an effect](extending/effect.md)
- [Add a transition](extending/transition.md)
- [Add a backend](extending/backend.md)

## Release procedures

- [Releasing the Python package](releasing.md)
