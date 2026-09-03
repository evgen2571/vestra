# Render with the CLI

The CLI works with canonical JSON project files. Use the three core commands
as a small workflow:

```bash
ve validate project.json
ve inspect project.json
ve render project.json --output preview.mp4 --overwrite
```

`validate` checks the loaded project and its usable resources. `inspect` prints
the loaded project and asset information without rendering it. `render` runs
preparation, frame evaluation, audio processing, and encoding.

Useful render options include:

```bash
ve render project.json \
  --output final.mp4 \
  --overwrite \
  --render-backend cpu \
  --progress terminal \
  --format json \
  --report render-report.json
```

The default output format is human-readable. Progress defaults to Auto: it is
shown on a supported interactive terminal and disabled for redirected or CI
 stderr. Use `--progress terminal` to force the terminal presentation,
`--progress json` for the event stream, or `--progress none` when another
program owns the display. `--format json` selects a machine-readable final
result. `--preview` requests preview rendering behavior.
`--render-backend` accepts `auto`, `cpu`, or `wgpu`.

Run `validate` first in scripts so an invalid project fails before encoding.
Use `--overwrite` only when replacing the output is intentional. A WGPU
request selects the WGPU path, not necessarily a hardware adapter. Check the
selected backend, adapter data, warnings, or report.

`generate-schema --output path.json` writes the current machine-readable
project schema when you need to inspect or regenerate that artifact.
