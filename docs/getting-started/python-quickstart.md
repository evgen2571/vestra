# Python quickstart

Create a one-second blue video, save it as an MP4 and play it back. This
example needs no images, fonts or other media files.

## Prerequisites

Install Vestra with `uv add vestra` in your Python project or
`python -m pip install vestra` in your chosen environment. Complete the
[installation guide](installation.md), including the `ffmpeg` and `ffprobe`
runtime checks.

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

# Fill the canvas with blue for the whole second.
project.root.add(Color("#2f6fed"), duration=1, id="background")

output = Path("quickstart.mp4")
result = project.render(output, backend="cpu", overwrite=True)
print(f"rendered {result.output_path} with {result.selected_backend}")
```

`Project` sets the video size, frame rate and duration. Add visual content to
`project.root`; each added item becomes a layer with its own timing. Here the
blue background lasts for the whole video.

## Render it

Run the file with the Python interpreter where Vestra is installed:

```bash
uv run python quickstart.py
# Or, in the environment where pip installed Vestra:
python quickstart.py
```

The command writes `quickstart.mp4` in the current directory. It is a 320×180,
24 fps, one-second video filled with the colour `#2f6fed`. The explicit `cpu`
backend makes the first render independent of WGPU adapter availability.

## Expected result

You should see a line naming `quickstart.mp4` and the selected CPU backend. The
file should exist and be playable by an MP4-capable video player.
If rendering fails, check that `ffmpeg` and `ffprobe` are available as described
in the [installation guide](installation.md), then see
[Python troubleshooting](../troubleshooting/python.md).

## Check a project before rendering

For larger projects, you can check layer timing and other project settings
without rendering a video:

```python
report = project.validate()
print(f"Project settings valid: {report.is_valid}")
```

The report's `is_valid` property tells you whether these checks passed.
Validation does not check media files, FFmpeg or graphics adapters; those are
checked during preparation or rendering. See
[rendering and preparation](../guides/python/rendering-and-preparation.md).

## Next steps

Continue with [projects and compositions](../guides/python/projects-and-compositions.md),
then use the [sources and layers guide](../guides/python/sources-and-layers.md)
to build a larger edit. The Python guides cover animation, effects,
transitions, audio, nesting, and preparation.
