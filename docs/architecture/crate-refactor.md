# Phase 1 crate refactor

The repository is now a transitional Cargo workspace:

```text
video-editor (root transitional package)
    -> video-editor-core
```

`video-editor-core` is an internal crate (`publish = false`). It owns the
deterministic project schema types, diagnostics, shared domain values,
timeline conversion, animation evaluation, pure project validation, output
settings, resource limits, and effect normalization. It intentionally has no dependency on WGPU,
FFmpeg, Clap, the root application, or terminal output.

The root package remains responsible for application orchestration, CPU and
WGPU renderers, media probing and FFmpeg invocation, CLI presentation, output
publication, and the environment preflight part of project loading. Preflight
checks file accessibility, image decoding, audio probing, source-duration
bounds, and backend availability; semantic diagnostics are produced by core.

During this transition, `video-editor::diagnostic`, `domain`, `timeline`,
`animation`, and project schema values are compatibility facades re-exporting
the core crate. These facades preserve current imports and are to be removed
when the final SDK crate is introduced.

The completed direction is always:

```text
CLI / application / renderer / media
                -> video-editor-core
```

Core must never depend in the reverse direction. Phase 1 is complete:
`video-editor-core::plan` owns the canonical render-plan model, compiler,
active schedule, logical effect-pass plan, and per-frame evaluator. The root
`plan` module is a documented compatibility façade only. Root preflight keeps
filesystem, image, audio, FFprobe, FFmpeg, and backend checks, then supplies
their resolved results through `PlanCompileInput`.

Phase 2 begins with extracting CPU/WGPU renderer and decoded-resource ownership
into `video-editor-render`, while preserving staged submission, readback,
backpressure, and backend selection behavior.

## Migration map

| Current module | Core ownership | Transitional root responsibility |
| --- | --- | --- |
| `diagnostic`, `domain`, `timeline`, `animation` | Canonical implementation | Compatibility re-exports |
| `project/model`, pure `project/validation` | Canonical schema and semantic rules | Filesystem/media/backend preflight and validated-resource handles |
| `plan/model`, `plan/compiler`, `plan/schedule`, `plan/evaluation`, logical effect passes | Canonical backend-neutral planning and evaluation in `video-editor-core::plan` | Root façade adapts `ValidatedProject` into `PlanCompileInput` |

`PlanCompileInput` has a narrow constructor and private source fields; it
accepts paths and probed durations as resolved values without accessing them.

The final target remains five crates:

```text
video-editor-core
video-editor-render
video-editor-media
video-editor
video-editor-cli
```
