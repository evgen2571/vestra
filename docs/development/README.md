<span id="vestra-development"></span>

# Contributing to Vestra

Use the [contribution guide](https://github.com/evgen2571/vestra/blob/develop/CONTRIBUTING.md)
to set up a checkout and prepare a pull request against `develop`. Then choose
the route that matches your change. Documentation preview needs no native engine build.

## Choose a task

| I want to… | Start here | Check the result |
| --- | --- | --- |
| Improve documentation or examples | [Documentation authoring](documentation.md) | `just docs-check` and `just site-check`; execute changed examples as described in the guide. |
| Fix a bug | [Architecture overview](architecture/overview.md), then the owning crate | Reproduce it with a focused test; use the [check chooser](testing.md#choose-checks-for-your-change). |
| Add a feature | [Extension guides](#extension-guides) and [project contracts](architecture/project-to-plan.md) | Test affected contracts, backends and public surfaces; see [Testing](testing.md). |
| Investigate rendering or speed | [Render pipeline](architecture/render-pipeline.md), [GPU validation](gpu-validation.md), [Performance](performance.md) | Inspect actual frames and report the backend, adapter and measurement conditions. |
| Prepare a release | [Release procedures](releasing.md) | Follow the maintainer checklist, including installed-wheel and license checks. |

For a larger feature or public API change, discuss the approach in an issue first.
The sections below map the implementation and specialist procedures.

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
- [Effect algorithms and numerical contracts](effect-algorithms.md)
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

## Working with coding agents

Agents should read [AGENTS.md](https://github.com/evgen2571/vestra/blob/develop/AGENTS.md)
and use [PLANS.md](https://github.com/evgen2571/vestra/blob/develop/PLANS.md)
for substantial work. The [Vestra skill index](https://github.com/evgen2571/vestra/blob/develop/.agents/skills/README.md)
routes tasks to relevant verification workflows. The [issue workflow](https://github.com/evgen2571/vestra/blob/develop/docs/agents/issue-tracker.md)
owns specs and dependencies; an execution plan tracks implementation of the selected scope.
