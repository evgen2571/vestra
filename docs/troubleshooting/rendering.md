# Rendering problems

Run `ve validate project.json` first. Validation reports canonical project/schema/semantic errors without reading assets or starting a renderer. Preflight failures come later and cover paths, media probes, FFmpeg, output readiness, and requested backend availability.

`VESTRA-OUTPUT-PATH` usually means the output is invalid or already exists. Pass `--overwrite` only when replacement is intended. A cancelled render raises/returns cancellation rather than a successful result. Compare requested and actual backend in the result when auto selection falls back. Media, encoder finalization, and publication failures have distinct diagnostic stages.
