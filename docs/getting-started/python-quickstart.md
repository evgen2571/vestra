# Python quickstart

This tutorial builds a one-second project from a solid colour and renders it
to an MP4. It uses the high-level editing API, so preparation and the native
render runtime stay behind `Project.render`.

## Prerequisites

Complete the [installation guide](installation.md). The commands below assume
you run them from the repository root with `uv run`.

## Create the project

Create a file named `quickstart.py` with this complete example:

```python
from pathlib import Path

import vestra
from vestra.sources import Color

project = vestra.Project(
    size=(320, 180),
    fps=24,
    duration=1,
)

# A Composition contains Layer placements. Color is the Source used here.
project.root.add(Color("#2f6fed"), duration=1, id="background")

report = project.validate()
if not report.is_valid:
    raise RuntimeError("project validation failed")

output = Path("quickstart.mp4")
result = project.render(output, backend="cpu", overwrite=True)
print(f"rendered {result.output_path} with {result.selected_backend}")
```

`Project` defines the canvas, frame rate, and duration. `project.root` is the
root `Composition`. Adding a `Source` creates a `Layer` in that composition.
The layer lasts one second, so it covers the complete project.

## Render it

Run the file with the environment created during installation:

```bash
uv run python quickstart.py
```

The command writes `quickstart.mp4` in the current directory. It is a 320×180,
24 fps, one-second video filled with the colour `#2f6fed`. The explicit `cpu`
backend makes the first render independent of WGPU adapter availability.

## Expected result

You should see a line naming `quickstart.mp4` and the selected CPU backend. The
file should exist and be playable by an MP4-capable video player. If validation
fails, inspect the report before rendering. If encoding fails, check the FFmpeg
runtime from the [installation guide](installation.md).

## Next steps

Return to the [documentation index](../index.md) for the next guides. The
existing repository examples show how to add images, video, shapes, text,
effects, transitions, and audio after this first render.
