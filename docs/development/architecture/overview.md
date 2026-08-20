# Architecture overview

Vestra separates project meaning from the code that renders or reads media. Put a rule about project JSON, timelines, effects, transitions, or signals in `vestra-core`. Put pixels and GPU state in `vestra-render`; put media formats, FFmpeg and output files in `vestra-media`. The `vestra` crate is the public Rust SDK that coordinates those layers. `vestra-python` exposes that SDK through PyO3, and `vestra-cli` presents the same SDK operations as `ve`.

```text
high-level Python ─┐
PyO3 bindings ────┼──> vestra SDK ──> core: model, validation, plan, evaluation
ve CLI ───────────┘          │
                               ├──> render: CPU or WGPU frames
                               └──> media: probe, decode, audio, encode, publish
```

The ordinary render path is `Project` load or lowering, semantic validation, target-specific preflight, plan and resource preparation, backend selection, frame and audio execution, temporary encoding, finalization, and publication. Loading and validation deliberately leave asset paths unresolved. Preflight resolves them and probes media. That separation lets a caller validate canonical JSON without touching the machine, while render operations can report environment failures before they start a frame loop.

`vestra-core` owns the canonical schema-v3 model, semantic validation, checked time and frame conversions, plan compilation, frame-time evaluation, animation, signal definitions, and effect/audio-effect descriptors. `vestra-render` consumes evaluated work; it must not reinterpret public project rules. CPU and WGPU are two implementations of the same prepared plan, not two project evaluators.

`vestra-media` owns the boundary where paths become media. It probes and decodes assets, builds and executes audio work, receives rendered RGBA frames through a sink, finalizes temporary encoder output, and publishes the requested file. A successful frame loop is therefore not a successful render until publication succeeds.

For the next level of detail, read [crate ownership](crates.md), [project to plan](project-to-plan.md), and [render pipeline](render-pipeline.md).
