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
machine-readable project schema. The [reference](reference/project-format.md)
pages document the exact project, API, CLI, feature, source, audio, diagnostic,
and backend contracts.

### Reference

- [Project format](reference/project-format.md)
- [Python API](reference/python-api.md)
- [Rust SDK](reference/rust-sdk.md)
- [CLI](reference/cli.md)
- [Feature support](reference/feature-support.md)
- [Diagnostics](reference/diagnostics.md)
- [Environment variables](reference/environment-variables.md)
- [Sources](reference/sources/image.md), [effects](reference/effects.md), [transitions](reference/transitions.md), [audio](reference/audio.md), and [backends](reference/backends.md)

## Something is failing?

Use the diagnostics printed by `ve validate` or `ve render`, then check the
repository tests and the [diagnostics reference](reference/diagnostics.md).

## Understanding or extending the engine?

The [backend concept](concepts/rendering-backends.md) explains the user-visible
CPU and WGPU choices. The [backend reference](reference/backends.md) records
selection and reporting semantics.

## Looking for old implementation reports?

Files under [`docs/history/`](history/) record audits, migrations, and other
implementation work as it was understood at the time. They are historical
records, not normative documentation for current Vestra behavior.
