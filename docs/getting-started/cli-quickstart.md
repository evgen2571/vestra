# CLI quickstart

This tutorial creates a minimal canonical project file, validates it, and
renders it with `ve`. The project uses schema version 3 and contains no external
assets, so it is easy to copy and inspect.

## Prerequisites

Complete the [installation guide](installation.md). The commands below use
`cargo run` so they work directly from a checkout. If `ve` is on your `PATH`,
replace `cargo run -q -p vestra-cli --` with `ve`.

## Create a project

Save this as `solid.json`:

```json
{
  "schema_version": 3,
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

The command should report `project is valid`. Validation checks the JSON shape,
schema version, project semantics, and the resources needed by the project.

## Render it

Render to an explicit output path and allow replacement of an existing file:

```bash
cargo run -q -p vestra-cli -- render solid.json \
  --output quickstart.mp4 \
  --overwrite \
  --progress none \
  --render-backend cpu
```

The output is `quickstart.mp4` in the current directory. It is a 320×180,
24 fps, one-second MP4 filled with `#2f6fed`. `--progress none` keeps this
minimal command quiet. Omit it to use the default human progress display.

The normal backend choice is `auto`. `cpu` is useful for a deterministic first
render. `wgpu` selects a WGPU adapter, which may be software rather than a
hardware GPU. The render result reports the selected backend.

## Inspect a failure

If validation fails, keep the project path and diagnostic text together when
checking the relevant source or test. For a machine-readable result, add
`--format json` to `validate` or `render`. `inspect` is available when you need
to examine the loaded project and its assets:

```bash
cargo run -q -p vestra-cli -- inspect solid.json
```

See the [documentation index](../index.md) for future CLI guides and the next
project-format reference.
