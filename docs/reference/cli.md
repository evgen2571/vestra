# CLI

The executable is `ve`. Use it to validate, inspect and render canonical JSON
projects. See [terminal rendering](../guides/cli/rendering.md) for the normal
workflow or [automation and logging](../guides/cli/logging-and-progress.md) for
scripts, reports and event streams.

```text title="Command signature summary"
ve [--verbose ...] COMMAND [OPTIONS]
```

## Global options and arguments

| Argument or option | Default | Contract |
| --- | --- | --- |
| `PROJECT` | Required by `validate`, `inspect` and `render`. | Path to a canonical JSON file. Relative asset paths use the project base directory. |
| `-v`, `--verbose` | WARN logging. | Repeat for INFO (`-v`), DEBUG (`-vv`) or TRACE (`-vvv`); accepted with any subcommand. |
| `-h`, `--help` | — | Show command help; `ve render --help` shows render options. |
| `-V`, `--version` | — | Print the CLI version. |

`RUST_LOG`, when set, supplies the tracing filter instead of the verbosity-derived
default.

## Commands

| Command | Arguments and options |
| --- | --- |
| `validate PROJECT` | `--format human\|json`, default `human`. Loads the project and runs the validation preflight target. |
| `inspect PROJECT` | `--preview`; `--format human\|json`, default `human`. |
| `render PROJECT` | Prepares, evaluates, encodes and publishes output; options below. |
| `generate-schema` | `--output PATH`, default `schemas/project.schema.json`. |
| `version` | Prints the Vestra version. |

## Results, diagnostics and logging

`PROJECT` is a path. Human results are short summaries: `validate` names the
valid project, `inspect` lists the resolved output, duration and contents, and
`render` reports the output path, size, frame count, renderer and elapsed time.
Diagnostics are printed as `error[CODE]: message` or `warning[CODE]: message`,
followed by the JSON location and hint when available. JSON results are
machine-readable envelopes; their fields are stable, the human text is not. Failures use a nonzero exit code and expose
diagnostics. Tracing is written to stderr. The default log policy is WARN;
`-v`, `-vv`, and `-vvv` enable INFO, DEBUG, and TRACE respectively. `RUST_LOG`
has precedence over those flags and supports target-specific filters such as
`vestra=warn,vestra.render.wgpu=trace`.

## Render

| Option | Default and values |
| --- | --- |
| `--output PATH` | Unset. The project output path is used. An override ending in `.gif` or `.webp` selects an animation. |
| `--overwrite` | False. Required to replace an existing output. |
| `--preview` | False. Caps longest side at 640 px, FPS at 15, and selects fast encoding. |
| `--format human\|json` | `human`. |
| `--progress auto\|terminal\|json\|none` | `auto`. JSON progress is JSONL. |
| `--report PATH` | Unset. Writes a structured report on success or failure; replaces a prior report only after the new report is written successfully. |
| `--render-backend auto\|cpu\|wgpu` | `auto`. |

### Report paths

Report paths must be separate from the project, referenced media and selected render
output, including normalized paths, symlinks and hard links. A collision fails
with `VESTRA-REPORT-PATH` before rendering or writing a failure report. Reports
are published from a temporary file; a failed write preserves a previous report.
If a project cannot be read or parsed, an existing report path is preserved
because protected media paths cannot be discovered; a new failure-report path
can still be used.

### Progress streams

With JSON progress and human final results, progress is sent to stdout and the
final result is sent to stderr to keep the streams separable. Auto selects
native terminal progress only for interactive supported stderr; redirected,
CI, dumb, and unsupported terminals are quiet. `terminal` forces the native
presentation. WGPU fallback is reported as `VESTRA-WGPU-FALLBACK` where
applicable.

| `--progress` | `--format` | stdout | stderr |
| --- | --- | --- | --- |
| `auto`, `terminal` or `none` | `human` | Human final result. | Tracing; terminal progress where enabled. |
| `auto`, `terminal` or `none` | `json` | One JSON final result envelope. | Tracing; terminal progress where enabled. |
| `json` | `human` | JSONL lifecycle events. | Human final result and tracing. |
| `json` | `json` | JSONL lifecycle events followed by the final result envelope. | Tracing. |

Events cover `started`, `stage_changed`, `progress` and a terminal outcome
(`completed`, `cancelled` or `failed`). A load or preflight failure can occur
before the lifecycle starts. Always check the exit code and final diagnostics.

Render JSON reports include the named CPU capacity gauges and worker/output
estimates described in [CPU memory accounting](../development/architecture/cpu-renderer.md#memory-accounting).
These are partial engine counters, not resident process memory.

## Validation meaning

`ve validate` uses `Editor.preflight` with `PreflightOptions::for_validation()`.
It can therefore report missing assets, FFmpeg/media failures, output
problems, and backend readiness issues. Python `project.validate()` and Rust
`Editor::validate()` perform canonical semantic validation only.

Validation uses the project's `output.path`, Auto backend selection and
overwrite disabled. An existing output causes `VESTRA-OUTPUT-PATH`; `validate`
has no `--overwrite` or `--output` option. For repeated editing, keep that path
unused and render to a separate file with `render --output preview.mp4 --overwrite`,
as in the [JSON guide](../guides/json-authoring.md).
`inspect` resolves project media without checking the render destination or
backend readiness.
