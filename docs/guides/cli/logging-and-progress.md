# Logging and progress

`ve` writes tracing logs to stderr. The default verbosity is WARN. Each `-v`
raises the default level:

```text
no -v   WARN
-v      INFO
-vv     DEBUG
-vvv+   TRACE
```

Set `RUST_LOG` when you need explicit tracing filters. It replaces the
verbosity-derived filter, so a command such as this can focus on one target:

```bash
RUST_LOG=vestra=warn,vestra.render.wgpu=trace ve render project.json --progress none
```

The stable application targets are:

```text
vestra
vestra.project
vestra.render
vestra.render.cpu
vestra.render.wgpu
vestra.media
vestra.media.probe
vestra.media.image
vestra.media.video
vestra.media.audio
vestra.encode
vestra.output
vestra.cache
vestra.performance
```

Examples:

```bash
RUST_LOG=vestra=info ve render project.json
RUST_LOG=vestra=warn,vestra.render.wgpu=debug ve render project.json
RUST_LOG=warn,vestra.performance=debug ve render project.json
```
Render records carry `operation_id`, `stage`, `output`, `requested_backend`,
`actual_backend`, `total_frames`, and elapsed timing fields where applicable.
The root render span uses the same `operation_id` as the native `RenderEvent`
stream, and child records inherit that context in both human and JSON output.

Log lines are formatted conceptually as:

```text
TIME LEVEL TARGET MESSAGE fields...
```

The exact field set depends on the event. Logs stay on stderr so stdout can be
used for command results or JSON progress.

Human tracing logs use stderr, or the configured file destination; they never
contaminate normal command stdout. The shared observability infrastructure
supports human-readable logs, JSON Lines logs, stderr, file, and stderr-plus-file
output. JSON tracing logs are separate from `--progress json`, which is a
`RenderEvent` stream.

## Progress modes

`render` accepts `--progress auto`, `--progress terminal`, `--progress json`,
and `--progress none`. Auto is the default. `terminal` forces the native
terminal presentation; `json` emits the machine-readable event stream; `none`
disables progress.

Human progress reports percentage and completed versus total frames. Once
enough samples exist it also shows rolling render FPS and an ETA. On a TTY it
updates a terminal line. Auto uses native terminal progress only with
interactive stderr; redirected stderr, CI, `TERM=dumb`, and unsupported
terminals receive no native human progress. Completion reports the rendered
frame count, elapsed time, and output path. Logs coordinate with the active
display so a log does not permanently destroy the progress line.

JSON progress writes one JSON object per line to stdout for the complete typed
lifecycle: `started`, `stage_changed`, `progress`, and one terminal event
(`completed`, `cancelled`, or `failed`). Use it for a process monitor or
pipeline. `--progress none` suppresses progress events while leaving logs and
the final command result under their normal format rules.

When `--progress json` is combined with `--format json`, progress events and the
final result form one JSONL stream on stdout. With `--format human`, the JSON
progress remains on stdout and the human final result goes to stderr, keeping
stdout machine-readable.
