# Vestra

Vestra is a Rust video editing and rendering engine with a Python authoring API
and a JSON-driven command-line renderer. You describe a project as compositions,
layers, and sources, then render it through the CPU or WGPU backend.

## What Vestra provides

- A high-level Python editing API built around `Project`, `Composition`,
  `Layer`, and `Source`.
- A canonical schema version 1 for projects that can be validated and rendered
  with the `ve` CLI.
- Sources for solid colours, images, video, shapes, text, particles, and other
  current project features.
- CPU rendering and WGPU rendering through the same project and render model.
- Structured validation, inspection, render results, diagnostics, and progress
  events in the Rust and Python SDKs.

Vestra 0.1.0 is the first public release. Project schema 1 and RenderEvent
schema 1 are its compatibility baseline.

## Quick start

Install with `pip install vestra`, then follow the [installation guide](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/installation.md).

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

The [Python quickstart](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/python-quickstart.md) explains
each part and shows how to check the result.

### CLI

Given a project file, validate it and render it with:

```bash
ve validate project.json
ve render project.json --output quickstart.mp4 --overwrite
```

The [CLI quickstart](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/cli-quickstart.md) includes a tiny
schema version 1 project you can copy and run.

## Rendering backends

Use the default `auto` choice unless you need a specific backend. `cpu` is the
most predictable option for a first render. `wgpu` selects a WGPU adapter, but
WGPU does not by itself mean hardware acceleration. The selected adapter and
any fallback are reported by the render result. Hardware validation is covered
in the [GPU validation guide](https://github.com/evgen2571/vestra/blob/main/docs/development/gpu-validation.md).

## Documentation

Start at the [documentation index](https://github.com/evgen2571/vestra/blob/main/docs/index.md), which routes readers by
task. Useful entry points include:

- [Installation](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/installation.md)
- [Python quickstart](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/python-quickstart.md)
- [CLI quickstart](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/cli-quickstart.md)
- [Authoring model](https://github.com/evgen2571/vestra/blob/main/docs/concepts/authoring-model.md)
- [Python rendering guide](https://github.com/evgen2571/vestra/blob/main/docs/guides/python/rendering-and-preparation.md)
- [CLI rendering guide](https://github.com/evgen2571/vestra/blob/main/docs/guides/cli/rendering.md)
- [Project format reference](https://github.com/evgen2571/vestra/blob/main/docs/reference/project-format.md)
- [Python API reference](https://github.com/evgen2571/vestra/blob/main/docs/reference/python-api.md)
- [Feature support matrix](https://github.com/evgen2571/vestra/blob/main/docs/reference/feature-support.md)
- [CLI reference](https://github.com/evgen2571/vestra/blob/main/docs/reference/cli.md)
- [Backend reference](https://github.com/evgen2571/vestra/blob/main/docs/reference/backends.md)
- [Signals reference](https://github.com/evgen2571/vestra/blob/main/docs/reference/signals.md)
- [Troubleshooting](https://github.com/evgen2571/vestra/blob/main/docs/troubleshooting/rendering.md)
- [Development documentation](https://github.com/evgen2571/vestra/blob/main/docs/development/architecture/overview.md)
- [Canonical project schema](https://github.com/evgen2571/vestra/blob/main/schemas/project.schema.json)
- [Runnable examples](https://github.com/evgen2571/vestra/tree/main/examples/)

## Development

The repository contains the Rust workspace, Python package, CLI, examples, and
tests. For the normal development checks, use the pinned environment and run:

```bash
uv sync --locked --extra dev
./scripts/check.sh
```

See the [documentation index](https://github.com/evgen2571/vestra/blob/main/docs/index.md) for the current navigation.
Historical implementation reports live under [`docs/history/`](https://github.com/evgen2571/vestra/tree/main/docs/history/)
and are not current product documentation.
