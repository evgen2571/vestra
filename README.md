# Vestra

Vestra is a Rust video editing and rendering engine with a Python authoring API
and a JSON-driven command-line renderer. You describe a project as compositions,
layers, and sources, then render it through the CPU or WGPU backend.

## What Vestra provides

- A high-level Python editing API built around `Project`, `Composition`,
  `Layer`, and `Source`.
- A canonical schema version 4 for projects that can be validated and rendered
  with the `ve` CLI. Valid schema-3 projects remain supported as legacy input.
- Sources for solid colours, images, video, shapes, text, particles, and other
  current project features.
- CPU rendering and WGPU rendering through the same project and render model.
- Structured validation, inspection, render results, diagnostics, and progress
  events in the Rust and Python SDKs.

Vestra is under active development. The source build is the supported way to
use the project today. Public APIs and project files may change as the engine
settles, so pin the repository revision when reproducibility matters.

## Quick start

Set up the repository by following the [installation guide](docs/getting-started/installation.md).

### Python

This small project renders one second of solid colour without an external
asset:

```python
from pathlib import Path

import vestra
from vestra.sources import Color

project = vestra.Project(
    size=(320, 180),
    fps=24,
    duration=1,
)
project.root.add(Color("#2f6fed"), duration=1, id="background")

project.render(Path("quickstart.mp4"), backend="cpu", overwrite=True)
```

The [Python quickstart](docs/getting-started/python-quickstart.md) explains
each part and shows how to check the result.

### CLI

Given a project file, validate it and render it with:

```bash
ve validate project.json
ve render project.json --output quickstart.mp4 --overwrite
```

The [CLI quickstart](docs/getting-started/cli-quickstart.md) includes a tiny
schema version 4 project you can copy and run. Valid schema-3 projects remain
loadable as legacy input.

## Rendering backends

Use the default `auto` choice unless you need a specific backend. `cpu` is the
most predictable option for a first render. `wgpu` selects a WGPU adapter, but
WGPU does not by itself mean hardware acceleration. The selected adapter and
any fallback are reported by the render result. Hardware validation is covered
in the [GPU validation guide](docs/development/gpu-validation.md).

## Documentation

Start at the [documentation index](docs/index.md), which routes readers by
task. Useful entry points include:

- [Installation](docs/getting-started/installation.md)
- [Python quickstart](docs/getting-started/python-quickstart.md)
- [CLI quickstart](docs/getting-started/cli-quickstart.md)
- [Authoring model](docs/concepts/authoring-model.md)
- [Python rendering guide](docs/guides/python/rendering-and-preparation.md)
- [CLI rendering guide](docs/guides/cli/rendering.md)
- [Project format reference](docs/reference/project-format.md)
- [Python API reference](docs/reference/python-api.md)
- [Feature support matrix](docs/reference/feature-support.md)
- [CLI reference](docs/reference/cli.md)
- [Backend reference](docs/reference/backends.md)
- [Signals reference](docs/reference/signals.md)
- [Troubleshooting](docs/troubleshooting/rendering.md)
- [Development documentation](docs/development/architecture/overview.md)
- [Canonical project schema](schemas/project.schema.json)
- [Runnable examples](examples/)

## Development

The repository contains the Rust workspace, Python package, CLI, examples, and
tests. For the normal development checks, use the pinned environment and run:

```bash
uv sync --locked --extra dev
./scripts/check.sh
```

See the [documentation index](docs/index.md) for the current navigation.
Historical implementation reports live under [`docs/history/`](docs/history/)
and are not current product documentation.
