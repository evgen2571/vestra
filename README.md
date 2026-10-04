# Vestra

Vestra is a native video composition and rendering engine with a Python
authoring API and a Rust rendering core. Describe compositions, layers and
sources, then render through CPU or WGPU.

[![A ten-second video rendered by Vestra](https://raw.githubusercontent.com/evgen2571/vestra/main/examples/showcase/readme-demo/preview.gif)](https://github.com/evgen2571/vestra/blob/main/examples/showcase/readme-demo/render.mp4)

[Watch the full render](https://github.com/evgen2571/vestra/blob/main/examples/showcase/readme-demo/render.mp4)
· [Read the Python source](https://github.com/evgen2571/vestra/blob/main/examples/showcase/readme-demo/main.py)
· [Asset credits](https://github.com/evgen2571/vestra/blob/main/examples/showcase/ASSETS.md)

## Quick start

Use Python **3.11–3.14** and install **ffmpeg and ffprobe** on `PATH`.
Prebuilt wheels cover Linux x86_64/aarch64, macOS Intel/Apple Silicon and
Windows x64. Wheel users do not need a Rust or C build toolchain.

For a uv project:

```bash
uv add vestra
uv run python example.py
```

Or install and run with pip:

```bash
python -m pip install vestra
python example.py
```

Save this as `example.py` to create `output.mp4`:

```python
from vestra import Project
from vestra.sources import Color, Rectangle

project = Project(size=(640, 360), fps=30, duration=2)
project.root.add(Color("#171d20"), duration=2)
bar = project.root.add(Rectangle(width=240, height=8, fill="#a9c4aa"), duration=2)
bar.transform.scale.keyframe(0, (0.05, 1))
bar.transform.scale.keyframe(2, (1, 1))
project.render("output.mp4", backend="cpu", overwrite=True)
```

The [installation guide](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/installation.md)
covers FFmpeg, source builds and the separately built `ve` CLI.

## Capabilities

- Python authoring with `Project`, `Composition`, `Layer` and typed sources;
  native Rust SDK and a JSON-driven CLI.
- Canonical project schema v1, validation, inspection, structured diagnostics
  and render progress events.
- Images, video, shapes, text, particles and audio-reactive Spectrum2D.
- Keyframes and signals, nested compositions, masks, track mattes, blend modes,
  effects and transitions.
- Audio mixing, trimming, fades, effects and analysis signals.
- CPU and WGPU backends. Adapter selection and fallback are reported; WGPU can
  use a software adapter. See [feature support](https://github.com/evgen2571/vestra/blob/main/docs/reference/feature-support.md)
  for the current backend limits.

Vestra 0.1.0 is the first public release, with project and RenderEvent schema v1.

## Examples

The [showcases](https://github.com/evgen2571/vestra/blob/main/examples/showcase/README.md)
include rendered videos, source and licensed assets in the checkout:

| Project | Watch | What it shows |
| --- | --- | --- |
| README demo | [10-second video](https://github.com/evgen2571/vestra/blob/main/examples/showcase/readme-demo/render.mp4) | Typography, real imagery, masks, nesting and a dissolve |
| After dark | [14-second video](https://github.com/evgen2571/vestra/blob/main/examples/showcase/real-media-edit/render.mp4) | Three footage selections, reframing, grading, titles and audio |
| Sound / form | [12-second video](https://github.com/evgen2571/vestra/blob/main/examples/showcase/audio-visualizer/render.mp4) | Spectrum, master-audio signals and restrained particles |
| Compose / reframe | [8-second video](https://github.com/evgen2571/vestra/blob/main/examples/showcase/compositing/render.mp4) | Nested shapes, an animated matte, masks and transforms |

The [reference examples](https://github.com/evgen2571/vestra/blob/main/examples/README.md)
remain small, deterministic projects for studying individual features.

## Documentation

Start with the [Python quickstart](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/python-quickstart.md),
[authoring model](https://github.com/evgen2571/vestra/blob/main/docs/concepts/authoring-model.md)
or [CLI quickstart](https://github.com/evgen2571/vestra/blob/main/docs/getting-started/cli-quickstart.md).
The [documentation index](https://github.com/evgen2571/vestra/blob/main/docs/index.md)
contains the guides, API references and troubleshooting pages.

## Development

```bash
nix develop
just python-sync
just style check python-test examples showcase-smoke
```

Run `just --list` for rendering, packaging, GPU and benchmark commands.
See [contributing](https://github.com/evgen2571/vestra/blob/main/docs/development/contributing.md)
and [testing](https://github.com/evgen2571/vestra/blob/main/docs/development/testing.md)
for native environments and verification.

## License

Vestra's software is [MIT licensed](https://github.com/evgen2571/vestra/blob/main/LICENSE).
Showcase images, footage, audio and fonts retain their
[documented asset licenses](https://github.com/evgen2571/vestra/blob/main/examples/showcase/ASSETS.md).

Native dependencies retain their own licenses; see
[third-party notices](https://github.com/evgen2571/vestra/blob/main/THIRD_PARTY_NOTICES.md).
