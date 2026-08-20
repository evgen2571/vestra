# CLI

The executable is `ve`. Global options precede a subcommand. `-v` can be
repeated. `RUST_LOG`, when set, supplies the tracing filter used by logging;
verbosity flags provide the CLI's default filter when it is unset.

## Commands

| Command | Arguments and options |
| --- | --- |
| `validate PROJECT` | `--format human\|json`, default `human`. Loads the project and runs the validation preflight target. |
| `inspect PROJECT` | `--preview`; `--format human\|json`, default `human`. |
| `render PROJECT` | Options below. |
| `generate-schema` | `--output PATH`, default `schemas/project.schema.json`. |
| `version` | Prints the Vestra version. |

`PROJECT` is a path. Human results are printed as readable output. JSON results
are machine-readable envelopes. Failures use a nonzero exit code and expose
diagnostics. Tracing is written to stderr.

## Render

| Option | Default and values |
| --- | --- |
| `--output PATH` | Unset. The project output path is used. |
| `--overwrite` | False. Required to replace an existing output. |
| `--preview` | False. Requests preview output behavior. |
| `--format human\|json` | `human`. |
| `--progress human\|json\|none` | `human`. JSON progress is JSONL. |
| `--report PATH` | Unset. Writes a structured report on success or failure. |
| `--render-backend auto\|cpu\|wgpu` | `auto`. |

With JSON progress and human final results, progress is sent to stdout and the
final result is sent to stderr to keep the streams separable. WGPU fallback is
reported as `VESTRA-WGPU-FALLBACK` where applicable.

## Validation meaning

`ve validate` uses `Editor.preflight` with `PreflightOptions::for_validation()`.
It can therefore report missing assets, FFmpeg/media failures, output
problems, and backend readiness issues. Python `project.validate()` and Rust
`Editor::validate()` perform canonical semantic validation only.
