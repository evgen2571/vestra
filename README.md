<h1 align="center">Vestra</h1>

<p align="center">Video composition with Python, JSON and Rust</p>

<p align="center">
  <a href="https://pypi.org/project/vestra/"><img src="https://img.shields.io/pypi/v/vestra?style=flat-square&amp;color=347860" alt="PyPI version"></a>
  <a href="docs/getting-started/installation.md"><img src="https://img.shields.io/pypi/pyversions/vestra?style=flat-square&amp;label=Python&amp;color=347860" alt="Supported Python versions"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-347860?style=flat-square" alt="License: MIT"></a>
</p>

Vestra is a video composition and rendering engine for making videos from code or
data. It handles layers, animation, effects and audio in a shared timeline, so you
can build reusable scenes instead of assembling every frame yourself. Use it for
motion graphics, audio visualizers or automated video generation, through Python,
JSON projects or the Rust SDK.

https://github.com/user-attachments/assets/9e7044ba-c4d5-49fc-a0aa-254eb1649b59

<p align="center">
  <a href="examples/showcase/readme-demo/main.py">Hero source</a> · <a href="examples/showcase/ASSETS.md">Asset credits</a>
</p>

[Showcase gallery](examples/showcase/README.md) · [Examples](examples/README.md) · [Documentation](docs/index.md)

## Usage

### Python

Requires Python 3.11–3.14 and `ffmpeg`/`ffprobe`. [Installation guide](docs/getting-started/installation.md).

```bash
python -m pip install vestra
```

Save this as `quickstart.py` and run `python quickstart.py`:

```python
from vestra import Interpolation, Project
from vestra.sources import Color, Rectangle

project = Project(size=(640, 360), fps=30, duration=3)
project.root.add(Color("#101c24"))
card = project.root.add(
    Rectangle(width=180, height=140, corner_radius=16, fill="#b8e3bc")
)
card.transform.position.keyframe(0, (0.25, 0.5))
card.transform.position.keyframe(1.5, (0.75, 0.5), interpolation=Interpolation.EASE_IN_OUT)
card.transform.position.keyframe(3, (0.25, 0.5), interpolation=Interpolation.EASE_IN_OUT)

project.render("quickstart.mp4", backend="cpu", overwrite=True)
```

<p align="center">
  <a href="examples/showcase/quickstart/render.mp4"><img src="examples/showcase/quickstart/preview.gif" width="480" alt="The Python example: a rounded rectangle moving with eased keyframes"></a>
</p>

[Full example](examples/showcase/quickstart/main.py) · [Python tutorial](docs/getting-started/python-quickstart.md)

### JSON

Store a composition as data and render it with the CLI. This example animates a
rounded rectangle; save it as `scene.json` or use the [full example](examples/projects/readme-animation.json).

<details>
<summary>JSON composition</summary>

```json
{
  "schema_version": 1,
  "output": {
    "path": "json-example.mp4", "width": 640, "height": 360,
    "frame_rate": "30/1", "background": "#101c24",
    "quality": "balanced", "audio": false,
    "duration_mode": "explicit", "duration": 3
  },
  "assets": [],
  "visual": {
    "clips": [{
      "id": "card", "start": 0, "duration": 3, "layer": 0,
      "source": {
        "type": "shape", "fill": "#b8e3bc",
        "geometry": {"type": "rectangle", "width": 180, "height": 140, "corner_radius": 16}
      },
      "opacity": {"base_value": 1},
      "transform": {
        "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
        "scale": {"base_value": {"x": 1, "y": 1}},
        "position": {
          "base_value": {"x": 0.25, "y": 0.5},
          "keyframes": [
            {"time": 0, "value": {"x": 0.25, "y": 0.5}, "interpolation": "linear"},
            {"time": 1.5, "value": {"x": 0.75, "y": 0.5}, "interpolation": "ease_in_out"},
            {"time": 3, "value": {"x": 0.25, "y": 0.5}, "interpolation": "ease_in_out"}
          ]
        }
      }
    }]
  }
}
```

</details>

From a source checkout with Rust and FFmpeg installed:

```bash
cargo run -p vestra-cli -- render scene.json --render-backend cpu --overwrite
```

The CLI is separate from the Python wheel. [CLI setup](docs/getting-started/cli-quickstart.md) · [JSON guide](docs/guides/json-authoring.md)

## Vestra vs MoviePy

| If your work centres on… | Vestra | [MoviePy](https://zulko.github.io/moviepy/) |
| :--- | :--- | :--- |
| Animated compositions | Built-in keyframes, nested compositions and particles suit reusable motion graphics. | Flexible clip functions suit edits with custom timing and animation logic. |
| Audio-driven visuals | Spectrum sources and audio-signal bindings connect sound to visual properties. | Python/NumPy gives you control over custom audio analysis and frame processing. |
| Automated pipelines | Python and JSON share a project model; CPU and WGPU rendering suit different hardware ([support](docs/reference/feature-support.md)). | A mature Python ecosystem makes it easy to combine video with other libraries. |
| Output and stability | Currently exports H.264/AAC MP4; the 0.1.x Python API is evolving. | A longer-established tool with broader output support. |

For measured performance, see the [workloads, methodology and benchmark results](docs/benchmarks/vestra-vs-moviepy.md).

<details>
<summary>Current limits</summary>

Rendering produces a file; there is no timeline UI. Text requires an explicit
font file. `auto` chooses WGPU when a usable adapter is available and otherwise
falls back to CPU. GPU rendering does not imply hardware decoding or encoding.
See [feature support](docs/reference/feature-support.md) and
[backends](docs/reference/backends.md).

</details>

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and contribution guidance.
The [development docs](docs/development/README.md) cover the
[architecture](docs/development/architecture/overview.md), [testing](docs/development/testing.md)
and [GPU validation](docs/development/gpu-validation.md).

## AI use disclaimer

Vestra is developed with extensive use of AI tools, including help with code,
tests and documentation. The project's direction, code quality and ongoing
maintenance remain the responsibility of its human maintainers. AI assistance
doesn't make a change correct: it still needs review, testing and care. If you
find a problem, please [report it](https://github.com/evgen2571/vestra/issues).

[MIT license](LICENSE) · [Software notices](THIRD_PARTY_NOTICES.md) · [Asset licenses](examples/showcase/ASSETS.md)
