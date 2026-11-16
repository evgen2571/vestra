# Rendering problems

Choose the symptom below and keep the diagnostic's code, JSON pointer and hint.
They identify the failed boundary more reliably than the last visible progress
percentage. For Python import errors, start with [Python problems](python.md).

## Canonical validation fails

**Check.** Inspect `ValidationReport.errors`: code, pointer, related ID and hint.
Python `project.validate()` and native `Editor.validate(snapshot)` check
canonical structure and semantic relationships without probing media or
initializing a renderer. High-level Python lowering resolves authored font
selections before canonical validation.

**Fix.** Correct the named field or relationship. Common causes are invalid
timing, duplicate IDs, transition endpoints in another composition, unsupported
effect scope and malformed signal bindings. Use the
[project format](../reference/project-format.md) for the exact contract.

**Confirm.** Run the same validation again and check that the report is valid.
Then prepare or render to exercise the separate media and environment checks.

## `ve validate` fails on a file or media tool

**Check.** `ve validate` runs validation-target preflight, which checks resources,
media, the configured destination and backend readiness. Read the JSON diagnostics:

```bash
ve validate project.json --format json
ve inspect project.json
```

`inspect` resolves the project summary without checking the render destination
or backend readiness. It can still fail on media it needs to inspect.

**Fix.** Correct the reported asset, tool or environment problem. If
`VESTRA-OUTPUT-PATH` names an existing configured output, keep that path unused
and render to a separate preview destination; `validate` has no overwrite
option. See [validation meaning](../reference/cli.md#validation-meaning).

**Confirm.** Rerun `validate` and then render to the intended destination. For a
missing executable, continue with [FFmpeg checks](ffmpeg.md).

## Output will not start or publish

**Check.** `VESTRA-OUTPUT-PATH` usually identifies an invalid destination or an
existing file without overwrite permission. If frames have already rendered,
read the reported failure stage: frame write, encoder finalization and
publication are separate operations.

**Fix.** Use a writable destination with enough free space. Add `--overwrite`
only when replacing the existing output is intentional. Preserve the reported
temporary and destination paths; do not treat the temporary file as a
successfully published result.

**Confirm.** Render to a fresh destination, wait for the successful command
result or terminal `completed` event, then open the published file. Frame
progress reaching 100% is not sufficient. See
[output publication](../reference/render-outputs.md#publication-and-failures).

## Backend, cancellation or opaque failure

**Check.** Compare requested and selected backend, fallback and adapter data in
`RenderResult` or the report. An unavailable explicit backend fails preflight;
Auto may choose CPU with `VESTRA-WGPU-FALLBACK`. A software WGPU adapter does not
establish hardware rendering. Cancellation returns a cancellation error.

**Fix.** For a backend failure, try CPU to isolate driver/device readiness from
project and media failures. For cancellation, check the token or interrupt
source. Add focused context with `ve -v render ...` or `RUST_LOG`; use
[WGPU troubleshooting](wgpu.md) for device-specific steps.

**Confirm.** Rerun with the intended backend and check the selected backend in
the successful result. A successful CPU render does not verify WGPU. For
cancellation, confirm that no successful result was returned and inspect the
reported cleanup outcome.

## Report an unresolved render failure

For a CLI reproduction, choose unused output and report paths, then capture:

```bash
ve version
ve -v render project.json --output reproduction.mp4 --progress none --format json --report reproduction-report.json
```

A successful report write replaces `reproduction-report.json`, including on
render failure. The report path must be separate from the project, media and
output; see [report protection](../reference/cli.md#report-paths).

Include these details in the issue:

| Evidence | What to include |
| --- | --- |
| Reproduction | Smallest project or script, exact command, expected result, actual result and whether it repeats. |
| Version and environment | Vestra version, OS/architecture, Python version if applicable, wheel versus source build and source commit for a checkout. |
| Failure | Full traceback or stderr, diagnostic codes/pointers/hints, final JSON result and report if available. |
| Backend | Requested and selected backend, fallback, adapter name/type, graphics API and driver details. |
| Media/output | A shareable failing asset or its format/stream facts, selected FFmpeg path and `-version` output when relevant, destination/temporary paths and available disk space. |

Remove credentials and private paths/media before sharing. Keep the diagnostic
structure and enough asset information to reproduce the failure. For meaning
of the fields, see [diagnostics](../reference/diagnostics.md).
