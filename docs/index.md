# Vestra documentation

Vestra is a source-built video editing and rendering engine. These pages are
organized around the task a reader is trying to complete.

## New to Vestra?

Begin with [installation](getting-started/installation.md), then follow the
[Python quickstart](getting-started/python-quickstart.md) or the
[CLI quickstart](getting-started/cli-quickstart.md).

## Understand the model

Start with the [authoring model](concepts/authoring-model.md), then read about
[time](concepts/timeline-and-time.md) and the [rendering lifecycle](concepts/rendering-lifecycle.md).

## Building videos in Python?

The [Python quickstart](getting-started/python-quickstart.md) covers the
high-level `Project` editing API and a first render. Continue with the
[Python guides](guides/python/projects-and-compositions.md) for sources,
animation, effects, audio, nesting, and rendering.

## Using the CLI?

Start with the [CLI quickstart](getting-started/cli-quickstart.md), then read
the [rendering guide](guides/cli/rendering.md) and
[logging and progress guide](guides/cli/logging-and-progress.md).

## Need exact behavior?

The checked-in [project schema](../schemas/project.schema.json) is the current
machine-readable project schema. Exact project-format and API contracts are
documented separately from this learning path.

## Something is failing?

Use the diagnostics printed by `ve validate` or `ve render`, then check the
repository tests and current development guidance. Troubleshooting pages will
cover common environment and media failures separately.

## Understanding or extending the engine?

The [backend concept](concepts/rendering-backends.md) explains the user-visible
CPU and WGPU choices. Development pages cover the Rust workspace, media
dependencies, and GPU validation.

## Looking for old implementation reports?

Files under [`docs/history/`](history/) record audits, migrations, and other
implementation work as it was understood at the time. They are historical
records, not normative documentation for current Vestra behavior.
