# Rendering problems

## Canonical validation fails

Use `Project.validate()` for a high-level Python project or `Editor.validate(snapshot)` for a native `ProjectSnapshot`. They check canonical structure and semantic relationships without probing media or initializing a renderer. High-level Python lowering resolves authored font selections before canonical validation. Inspect `ValidationReport.errors` and each diagnostic's code, pointer, related ID and hint. Typical causes are invalid timing, duplicate IDs, invalid transition endpoints, unsupported effect scope or a malformed signal binding.

## `ve validate` fails on a file or media tool

`ve validate project.json` is different. It runs `Editor.preflight` with `PreflightOptions::for_validation()`, so it may resolve assets, probe media, check FFmpeg-related readiness and report environment failures. It also rejects an existing configured output; see [validation meaning](../reference/cli.md#validation-meaning) for a repeatable workflow. Use `ve inspect project.json` to see a derived summary, and `ve validate project.json --format json` when another tool needs diagnostics.

## Output will not start or publish

An invalid destination or an existing output without `--overwrite` fails before rendering. `VESTRA-OUTPUT-PATH` usually identifies this boundary. Pass `--overwrite` only when replacement is intended. If frames render but the command fails later, distinguish frame write, encoder finalization and output publication from the reported failure stage. Include both destination and temporary path from the report when diagnosing a publication failure.

## Backend, cancellation or opaque failure

An unavailable requested backend fails preflight; an automatic WGPU request may fall back to CPU and report `VESTRA-WGPU-FALLBACK`. Check requested versus actual backend in `RenderResult` or the JSON report. Cancellation is cooperative and produces a cancellation error, not a partial success. For extra context run `ve -v render ...` or set `RUST_LOG` to an appropriate tracing filter. The [backends](../reference/backends.md), [diagnostics](../reference/diagnostics.md) and [WGPU troubleshooting](wgpu.md) pages explain the reported facts.
