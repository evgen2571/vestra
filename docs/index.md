# Vestra documentation

Vestra is a source-built video editing and rendering engine. These pages are
being reorganized around the task a reader is trying to complete.

## New to Vestra?

Begin with [installation](getting-started/installation.md), then follow the
[Python quickstart](getting-started/python-quickstart.md) or the
[CLI quickstart](getting-started/cli-quickstart.md).

## Building videos in Python?

The [Python quickstart](getting-started/python-quickstart.md) covers the
high-level `Project` editing API and a first render. Python guides and exact
API reference pages will be added here as the documentation rewrite continues.

## Using the CLI?

Start with the [CLI quickstart](getting-started/cli-quickstart.md). It covers a
minimal project, validation, and rendering. More detailed CLI guides and
reference pages will follow.

## Need exact behavior?

The checked-in [project schema](../schemas/project.schema.json) is the current
machine-readable contract. More reference pages are planned for the project
format, Python API, Rust SDK, sources, and backends.

## Something is failing?

Use the diagnostics printed by `ve validate` or `ve render`, then check the
repository tests and current development guidance. Dedicated troubleshooting
pages are planned.

## Understanding or extending the engine?

Development and architecture pages are planned for the Rust workspace, media
dependencies, renderer backends, and GPU validation.

## Looking for old implementation reports?

Files under [`docs/history/`](history/) record audits, migrations, and other
implementation work as it was understood at the time. They are historical
records, not normative documentation for current Vestra behavior.
