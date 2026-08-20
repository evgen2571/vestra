# Contributing

Start with the smallest change that makes the intended behavior true. Preserve the current crate boundaries: canonical model and semantics in core, pixels/resources in render, FFmpeg and output paths in media, orchestration in the SDK, and presentation in CLI/Python. Use CodeGraph for an unfamiliar execution path, then inspect the named source and tests rather than searching the whole tree by habit.

Several interfaces are compatibility-sensitive: public Rust and Python APIs, `_native.pyi`, canonical JSON/schema, CLI options and reports, diagnostics, time/frame/sample conversion and published output behavior. A source/effect/transition change usually needs model, compiler/evaluator, CPU, WGPU, Python/lowering, schema and reference work. Do not claim CPU/WGPU parity from matching dispatch arms alone.

Keep media and process details behind `vestra-media`; callers should ask the SDK to preflight and render rather than reimplementing FFmpeg checks. Keep validation and preflight distinct. Semantic `validate()` does not read assets, while render-target preflight may.

Before proposing a change, run formatting plus focused checks and use the relevant broader command from [Testing](testing.md). Recheck generated schema, public exports/stubs and current CLI help when those contracts change. Hardware WGPU claims require the procedure in [GPU validation](gpu-validation.md). Update current documentation in the same change when user-visible behavior changes, and inspect the final diff for accidental unrelated edits.
