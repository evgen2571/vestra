# Render with the CLI

Render a canonical JSON project from your terminal. You need the separate
[`ve` executable](../../getting-started/cli-installation.md), FFmpeg on `PATH`,
and a project with accessible media. The [CLI quickstart](../../getting-started/cli-quickstart.md)
provides a complete project if you do not have one yet.

## Check the project

```bash
ve validate project.json
ve inspect project.json
```

`validate` checks the project, media, configured output and backend readiness.
`inspect` shows the resolved duration, output and asset information without
rendering. Neither command creates a video.

Keep the project's configured output path unused for repeated validation.
Validation always checks
the configured destination with overwrite disabled. See
[validation meaning](../../reference/cli.md#validation-meaning).

## Make a preview

The following command replaces `preview.mp4` in the current directory:

```bash
ve render project.json --output preview.mp4 --preview --render-backend cpu --overwrite
```

Preview mode caps the longest side at 640 pixels and FPS at 15, and uses fast
encoding. It leaves the JSON project unchanged. CPU needs no graphics driver.
The normal terminal display shows progress and finishes with the output path,
frame count, renderer and elapsed time. Open `preview.mp4` to check the result.

## Render at project settings

Remove preview mode and select a final destination. This command refuses an
existing `final.mp4`; add `--overwrite` when replacement is intentional:

```bash
ve render project.json --output final.mp4
```

Auto backend selection tries WGPU and can fall back to CPU. Check the reported
selected backend and warnings; WGPU may use a software adapter. See
[backend selection](../../reference/backends.md) when requesting a specific device path.

An output ending in `.gif` or `.webp` selects a silent looping animation.
Use [render outputs](../../reference/render-outputs.md) for format limits and
the [CLI reference](../../reference/cli.md) for all option values and defaults.

For scripts, reports and live event streams, continue with
[CLI automation and logging](logging-and-progress.md).
