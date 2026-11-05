# CLI quickstart

This tutorial creates a small JSON project file, validates it, and
renders it with `ve`. The project uses schema version 1 and contains no
external assets, so it is easy to copy and inspect.

## Prerequisites

The Python package does not install the `ve` CLI. Build the CLI separately
from a repository checkout using the
[build-from-source instructions](installation.md#build-from-source--development).
You also need `ffmpeg` and `ffprobe` on `PATH`.

Run the commands below from the checkout. They use `cargo run` to build and
execute the CLI. If you have already added `ve` to your `PATH`, replace
`cargo run -q -p vestra-cli --` with `ve`.

## Create a project

Save this as `solid.json`:

```json
{
  "schema_version": 1,
  "name": "Minimal solid project",
  "output": {
    "path": "output.mp4",
    "width": 320,
    "height": 180,
    "frame_rate": "24/1",
    "background": "#000000",
    "quality": "preview",
    "audio": false,
    "duration_mode": "explicit",
    "duration": 1
  },
  "assets": [],
  "visual": {
    "clips": [
      {
        "id": "background",
        "source": {"type": "solid_color", "colour": "#2f6fed"},
        "start": 0,
        "duration": 1,
        "layer": 0,
        "opacity": {"base_value": 1}
      }
    ]
  }
}
```

The `output` object describes the render size, frame rate, duration, and
background. The visual clip places one solid-colour source on layer 0 for the
whole second. This is enough for the CLI to build and render a project.

## Validate the project

Run validation before rendering:

```bash
cargo run -q -p vestra-cli -- validate solid.json
```

The command should print `solid.json is valid`. Validation checks the JSON
shape, schema version, timing and layer settings, plus the media files, output
path and renderer the project needs. Problems are reported with a code, the
JSON location and, where available, a hint:

```text
error[VESTRA-KEYFRAME-TIME]: keyframe times must be finite, strictly increasing, and inside the clip
  at /visual/clips/0/opacity/keyframes/0/time
```

Keep the configured `output.mp4` path unused while repeating validation: the
command rejects an existing destination. The render below writes to
`quickstart.mp4` instead. See [validation meaning](../reference/cli.md#validation-meaning).

## Render it

Render to an explicit output path and allow replacement of an existing file:

```bash
cargo run -q -p vestra-cli -- render solid.json \
  --output quickstart.mp4 \
  --overwrite \
  --progress none \
  --render-backend cpu
```

The command prints a summary such as:

```text
Rendered quickstart.mp4
  320x180 at 24 fps, 1.00 s (24 frames)
  cpu renderer, finished in 0.21 s
```

The output is `quickstart.mp4` in the current directory, a one-second MP4
filled with `#2f6fed`. `--progress none` keeps this
minimal command quiet. Progress defaults to Auto: it is shown on an interactive
terminal and stays quiet when stderr is redirected, in CI, or under
`TERM=dumb`. Use `--progress terminal` to force the terminal presentation.

The normal backend choice is `auto`. `cpu` is useful for a deterministic first
render. `wgpu` selects a WGPU adapter, which may be software rather than a
hardware GPU. The render result reports the selected backend.

## Inspect a failure

Read the diagnostic message for the setting or asset that needs attention.
Check that `solid.json` contains the complete example above. If rendering fails,
check `ffmpeg -version` and `ffprobe -version`, then consult
[FFmpeg troubleshooting](../troubleshooting/ffmpeg.md) or
[rendering troubleshooting](../troubleshooting/rendering.md).

Use `inspect` to see the resolved output settings, duration and contents:

```bash
cargo run -q -p vestra-cli -- inspect solid.json
```

```text
solid.json (Minimal solid project)
  output   output.mp4
           320x180 at 24 fps, 1.00 s (24 frames, explicit duration)
  visual   1 clip, 0 transitions, 0 flashes
  assets   0 images, 0 audio files
  audio    none
```

For scripts that need machine-readable diagnostics, add `--format json` to
`validate` or `render`. When reporting a problem, include the command, a small
project that reproduces it and the diagnostic message.

Continue with [Compose videos with JSON](../guides/json-authoring.md) to add
layers, animation, transitions and audio. See the
[CLI rendering guide](../guides/cli/rendering.md) and the
[logging and progress guide](../guides/cli/logging-and-progress.md). See the
[documentation index](../index.md) for the current learning path.
