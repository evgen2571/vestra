# CLI

The executable is `ve`. Global options precede a subcommand. `-v` can be
repeated. `RUST_LOG`, when set, supplies the tracing filter used by logging;
verbosity flags provide the CLI's default filter when it is unset.

## Commands

| Command | Arguments and options |
| --- | --- |
| `validate PROJECT` | `--strict-audio`; `--format human\|json`, default `human`. Loads the project and runs the validation preflight target. |
| `inspect PROJECT` | `--strict-audio`; `--preview`; `--format human\|json`, default `human`. |
| `render PROJECT` | Options below. |
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
| `--output PATH` | Unset. The project output path is used. |
| `--overwrite` | False. Required to replace an existing output. |
| `--preview` | False. Requests preview output behavior. |
| `--format human\|json` | `human`. |
| `--progress auto\|terminal\|json\|none` | `auto`. JSON progress is JSONL. |
| `--report PATH` | Unset. Writes a structured report on success or failure. |
| `--render-backend auto\|cpu\|wgpu` | `auto`. |

### Progress streams

With JSON progress and human final results, progress is sent to stdout and the
final result is sent to stderr to keep the streams separable. Auto selects
native terminal progress only for interactive supported stderr; redirected,
CI, dumb, and unsupported terminals are quiet. `terminal` forces the native
presentation. WGPU fallback is reported as `VESTRA-WGPU-FALLBACK` where
applicable.

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

Audio inspection uses native first-stream metadata by default; JSON reports expose
`duration_source` without warning merely for reported metadata. Use
`ve validate PROJECT --strict-audio` or `ve inspect PROJECT --strict-audio` for
full-source duration and decoder checks. Inspection falls back to decoding when
metadata is unavailable. Rendering resolves omitted source ends exactly; explicit
trim ends use source-bound checks integrated into actual reading. See the
[audio duration contract](../development/architecture/media-io.md#audio-duration).
