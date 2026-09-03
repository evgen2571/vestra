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

The stable application targets are rooted at `vestra`: `vestra.project`,
`vestra.render`, `vestra.render.cpu`, `vestra.render.wgpu`, `vestra.media`,
`vestra.media.probe`,
`vestra.media.audio`, `vestra.media.video`, `vestra.media.image`,
`vestra.encode`, `vestra.output`, `vestra.cache`, and `vestra.performance`.
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

## Progress modes

`render` accepts `--progress human`, `--progress json`, and `--progress none`.
The default is `human`.

Human progress reports percentage and completed versus total frames. Once
enough samples exist it also shows rolling render FPS and an ETA. On a TTY it
updates a terminal line. Auto mode disables the built-in progress sink when
stderr is redirected, in CI, or under a dumb terminal; use `--progress human`
to request it explicitly. Completion reports the rendered frame count, elapsed
time, and output path. Logs coordinate with the active display so a log does
not permanently destroy the progress line.

JSON progress writes one JSON object per line to stdout for the started,
progress, and completed events. This is a JSONL stream, not one JSON document.
Use it for a process monitor or pipeline. `--progress none` suppresses progress
events while leaving logs and the final command result under their normal
format rules.

When `--progress json` is combined with `--format json`, progress events and the
final result form one JSONL stream on stdout. With `--format human`, the JSON
progress remains on stdout and the human final result goes to stderr, keeping
stdout machine-readable.
