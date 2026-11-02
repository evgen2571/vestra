<h1 align="center">Vestra</h1>

<p align="center">Video composition with Python, JSON and Rust</p>

<p align="center">
  <a href="https://pypi.org/project/vestra/"><img src="https://img.shields.io/pypi/v/vestra?style=flat-square&amp;color=347860" alt="PyPI version"></a>
  <a href="docs/getting-started/installation.md"><img src="https://img.shields.io/pypi/pyversions/vestra?style=flat-square&amp;label=Python&amp;color=347860" alt="Supported Python versions"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-347860?style=flat-square" alt="License: MIT"></a>
</p>

Vestra is a library for making videos with code. Think animated titles, audio
visualizers, or a scene you want to reuse with different text and images. You can
build it in Python or describe it in JSON, then keep it in version control like
the rest of your project.

https://github.com/user-attachments/assets/9e7044ba-c4d5-49fc-a0aa-254eb1649b59

<p align="center">
  <a href="examples/showcase/readme-demo/main.py">Hero source</a> · <a href="examples/showcase/ASSETS.md">Asset credits</a>
</p>

[Showcase gallery](examples/showcase/README.md) · [Examples](examples/README.md) · [Documentation](docs/index.md)

## Usage

### Python

You’ll need Python 3.11–3.14 and FFmpeg. [Setup instructions](docs/getting-started/installation.md).

```bash
python -m pip install vestra
```

Save this as `quickstart.py`, then run `python quickstart.py`:

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

[Python guide](docs/getting-started/python-quickstart.md)

### JSON

The same animation in JSON, saved as `scene.json`:

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

From a [source checkout](docs/getting-started/cli-quickstart.md):

```bash
cargo run -p vestra-cli -- render scene.json --render-backend cpu --overwrite
```

[Example file](examples/projects/readme-animation.json) · [JSON guide](docs/guides/json-authoring.md)

## Vestra vs MoviePy

| Vestra is a good fit for… | [MoviePy](https://zulko.github.io/moviepy/) is a good fit for… |
| :--- | :--- |
| Motion graphics with keyframes, nested compositions and particles. | Edits that need custom Python/NumPy frame processing. |
| Binding animation to audio signals, or reusing scenes through Python or JSON. | Scripts that combine video with the wider Python ecosystem. |
| [CPU or WGPU rendering](docs/reference/backends.md), if MP4 output is enough. The API is still young ([current support](docs/reference/feature-support.md)). | A mature tool with broader output support and flexible clip manipulation. |

[Performance benchmarks and methodology](docs/benchmarks/vestra-vs-moviepy.md)

## Development

[CONTRIBUTING.md](CONTRIBUTING.md) covers setup and sending a patch. The
[development docs](docs/development/README.md) cover architecture, testing and
GPU validation.

## AI use disclaimer

I use LLMs a lot while building Vestra. They help me write code, tests and docs,
but I decide what belongs here, and I’m responsible for the quality of the
project and for maintaining it.

I’m still learning, and I’ll make mistakes. I care about making Vestra useful
and the code understandable. If you spot a bug, a questionable design choice
or an explanation that doesn’t make sense, please
[point it out](https://github.com/evgen2571/vestra/issues). I appreciate people
taking the time to do that.

[MIT license](LICENSE) · [Software notices](THIRD_PARTY_NOTICES.md) · [Asset licenses](examples/showcase/ASSETS.md)
