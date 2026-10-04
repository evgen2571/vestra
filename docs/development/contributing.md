# Contributing

Enter the pinned environment with `nix develop`, then run `just` to list the
development commands. Use `just python-sync` to build the native Python package
and `just check` for the canonical contributor checks. CI uses the same recipes
for formatting, linting, Python tests, schema checks, and wheel smoke tests. The
recipes delegate to Cargo, uv, and the existing verification scripts.

Start with the smallest change that makes the intended behavior true. Preserve the current crate boundaries: canonical model and semantics in core, pixels/resources in render, FFmpeg and output paths in media, orchestration in the SDK, and presentation in CLI/Python. Use CodeGraph for an unfamiliar execution path, then inspect the named source and tests rather than searching the whole tree by habit.

Several interfaces are compatibility-sensitive: public Rust and Python APIs, `_native.pyi`, canonical JSON/schema, CLI options and reports, diagnostics, time/frame/sample conversion and published output behavior. A source/effect/transition change usually needs model, compiler/evaluator, CPU, WGPU, Python/lowering, schema and reference work. Do not claim CPU/WGPU parity from matching dispatch arms alone.

Keep media and process details behind `vestra-media`; callers should ask the SDK to preflight and render rather than reimplementing FFmpeg checks. Keep validation and preflight distinct. Semantic `validate()` does not read assets, while render-target preflight may.

Before proposing a change, run formatting plus focused checks and use the relevant broader command from [Testing](testing.md). Recheck generated schema, public exports/stubs and current CLI help when those contracts change. Hardware WGPU claims require the procedure in [GPU validation](gpu-validation.md). Update current documentation in the same change when user-visible behavior changes, and inspect the final diff for accidental unrelated edits.

Before opening a pull request, describe the concrete behavior change and checks
you ran. Use Issues for reproducible bugs and focused feature proposals; include
a minimal project, expected/actual output and relevant diagnostic codes. Redact
personal paths and environment details from logs. For GPU reports, include only
the adapter/backend facts needed to reproduce the issue.

Rendering changes need meaningful pixel/media checks in addition to compilation.
See [testing](testing.md) and [GPU validation](gpu-validation.md). Preserve public
Rust/Python APIs and schema v1 unless the change explicitly revises their contract.
Software is licensed under [MIT](../../LICENSE); third-party example assets retain
their own licenses and attribution requirements.
