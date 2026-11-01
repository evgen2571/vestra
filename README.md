<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/brand/vestra-dark.svg">
    <img src="docs/assets/brand/vestra-light.svg" width="280" height="56" alt="Vestra">
  </picture>
</p>

<h1 align="center">Video composition with Python and Rust</h1>

<p align="center">
  Vestra is a video composition and rendering engine.<br>
  It provides a Python API, JSON projects, and CPU and WGPU renderers.
</p>

<p align="center">
  <a href="https://pypi.org/project/vestra/"><img src="https://img.shields.io/pypi/v/vestra?style=flat-square&amp;color=347860" alt="PyPI version"></a>
  <a href="docs/getting-started/installation.md"><img src="https://img.shields.io/pypi/pyversions/vestra?style=flat-square&amp;label=Python&amp;color=347860" alt="Supported Python versions"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-347860?style=flat-square" alt="License: MIT"></a>
</p>

<p align="center">
  <a href="#usage">Getting started</a> · <a href="docs/index.md">Documentation</a> · <a href="examples/showcase/README.md">Showcases</a>
</p>

https://github.com/user-attachments/assets/acf3df91-ce40-4b98-91ff-f0fe31963d11

<p align="center">
  <a href="examples/showcase/readme-demo/main.py">Hero source</a> · <a href="examples/showcase/ASSETS.md">Asset credits</a>
</p>

<table>
  <tr>
    <td width="50%" align="center" valign="top">
      <strong>Video editing</strong><br><br>
      <a href="examples/showcase/real-media-edit/render.mp4"><img src="examples/showcase/real-media-edit/preview.gif" width="400" alt="City footage with animated framing, colour grading and dissolves"></a>
      <p>Three video clips with animated framing and dissolves.<br><a href="examples/showcase/real-media-edit/main.py">Python source</a></p>
    </td>
    <td width="50%" align="center" valign="top">
      <strong>Visual effects</strong><br><br>
      <a href="examples/showcase/lookbook/render.mp4"><img src="examples/showcase/lookbook/preview.gif" width="400" alt="A landscape with photographic, halftone and ASCII treatments"></a>
      <p>A photograph with halftone and ASCII effects.<br><a href="examples/showcase/lookbook/main.py">Python source</a></p>
    </td>
  </tr>
  <tr>
    <td width="50%" align="center" valign="top">
      <strong>Audio reactivity</strong><br><br>
      <a href="examples/showcase/audio-visualizer/render.mp4"><img src="examples/showcase/audio-visualizer/preview.gif" width="400" alt="A radial audio spectrum, pulsing ring and particles"></a>
      <p>Audio signals control a spectrum, ring and particles.<br><a href="examples/showcase/audio-visualizer/main.py">Python source</a> · <a href="examples/showcase/audio-visualizer/render.mp4">Video with sound</a></p>
    </td>
    <td width="50%" align="center" valign="top">
      <strong>Compositing</strong><br><br>
      <a href="examples/showcase/compositing/render.mp4"><img src="examples/showcase/compositing/preview.gif" width="400" alt="Animated circular and rectangular photo cutouts on a cream background"></a>
      <p>Masks and nested compositions with grouped animation.<br><a href="examples/showcase/compositing/main.py">Python source</a></p>
    </td>
  </tr>
</table>

All videos were rendered with Vestra on the CPU. Gallery GIFs are silent previews;
click one for the full MP4. [More examples](examples/README.md).

## Usage

**Python.** Requires Python 3.11–3.14, plus `ffmpeg` and `ffprobe` on `PATH`.
[Installation](docs/getting-started/installation.md).

```bash
python -m pip install vestra
```

Save this as `quickstart.py` and run `python quickstart.py`:

```python
from vestra import Interpolation, Project
from vestra.sources import Color, Rectangle

project = Project(size=(640, 360), fps=30, duration=3)
project.root.add(Color("#101c24"))
ease = Interpolation.EASE_IN_OUT

for index, colour in enumerate(("#527e79", "#b8e3bc", "#f1efe5")):
    card = project.root.add(
        Rectangle(width=140, height=180, corner_radius=16, fill=colour)
    )
    card.transform.position = (0.35 + index * 0.15, 0.5)
    angle = (index - 1) * 25
    card.transform.rotation_degrees.keyframe(0, angle)
    card.transform.rotation_degrees.keyframe(1.5, -angle, interpolation=ease)
    card.transform.rotation_degrees.keyframe(3, angle, interpolation=ease)

project.render("quickstart.mp4", backend="cpu", overwrite=True)
```

<p align="center">
  <a href="examples/showcase/quickstart/render.mp4"><img src="examples/showcase/quickstart/preview.gif" width="480" alt="The Python example: three rounded cards with eased rotation"></a>
</p>

This writes a three-second, 640×360 MP4 without external assets. Positions are
relative to the canvas; keyframe times are in seconds.
[Python tutorial](docs/getting-started/python-quickstart.md).

**JSON.** A project can also be stored as data, which lets templates, services and
applications in other languages generate videos without running Python. JSON
and Python use the same project model and rendering engine.

The complete example below animates a rounded rectangle across the frame and
back. Save it as `scene.json`, or use the
[checked-in example](examples/projects/readme-animation.json).

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

From a source checkout, build the separate CLI, validate the project and render it:

```bash
cargo build -p vestra-cli
target/debug/ve validate scene.json
target/debug/ve render scene.json --render-backend cpu \
  --output json-example.mp4 --overwrite
```

The Python wheel does not install the CLI.
[JSON authoring](docs/guides/json-authoring.md) · [CLI setup and usage](docs/getting-started/cli-quickstart.md).

## MoviePy comparison

Vestra includes property keyframes, nested compositions, particles and audio-signal
bindings. [MoviePy v2](https://zulko.github.io/moviepy/) has a longer track record,
flexible Python/NumPy frame processing and more output formats. Vestra currently
exports MP4 (H.264 + AAC), and its 0.1.x Python API is still evolving.

These are the existing **2026-10-31** measurements on a **Ryzen 5 3600, 16 GiB RAM
and GTX 1650 SUPER (Vulkan)**, running Linux. Vestra 0.1.3 and MoviePy 2.2.1 rendered
the same 640×360 / 30 fps timelines with libx264 CRF 23 / veryfast, without audio.
Each cell is **render time in seconds / peak process-tree RSS in MiB**, with both
values reported as medians of three runs after one warmup. Times include process
startup, decoding, rendering and encoding; RSS excludes GPU VRAM.

| System | Workload | Vestra CPU | Vestra WGPU | MoviePy v2 |
| :--- | :--- | ---: | ---: | ---: |
| GTX 1650 SUPER | Basic (10 s) | 0.835 / 445.9 | 1.064 / 567.9 | 0.997 / 213.2 |
| GTX 1650 SUPER | Editing (12 s) | 1.759 / 567.1 | 1.239 / 636.7 | 8.652 / 379.6 |
| GTX 1650 SUPER | Layered (10 s) | 1.789 / 599.5 | 1.305 / 668.5 | 4.227 / 365.3 |

Vestra used more memory in every case. On the basic workload, WGPU was slower
than both Vestra CPU and MoviePy.
[Workloads, methodology and full results](docs/benchmarks/vestra-vs-moviepy.md).

<details>
<summary>Current limits</summary>

Rendering produces a file; there is no timeline UI. Text requires an explicit
font file. `auto` chooses WGPU when a usable adapter is available and otherwise
falls back to CPU. GPU rendering does not imply hardware decoding or encoding.
See [feature support](docs/reference/feature-support.md) and
[backends](docs/reference/backends.md).

</details>

<p align="center">
  <a href="docs/reference/python-api.md">API reference</a> · <a href="CONTRIBUTING.md">Contributing</a> · <a href="https://github.com/evgen2571/vestra/issues">Issues</a><br>
  <sub><a href="LICENSE">MIT</a> · <a href="THIRD_PARTY_NOTICES.md">Software notices</a> · <a href="examples/showcase/ASSETS.md">Asset licenses</a></sub>
</p>
