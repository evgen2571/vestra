# Phase 3 crate refactor

The repository is now a transitional Cargo workspace:

```text
video-editor (root transitional package)
    -> video-editor-core
    -> video-editor-render -> video-editor-core
    -> video-editor-media -> video-editor-render (contracts only)
                         -> video-editor-core (encoder and audio settings)
```

`video-editor-core` is an internal crate (`publish = false`). It owns the
deterministic project schema types, diagnostics, shared domain values,
timeline conversion, animation evaluation, pure project validation, output
settings, resource limits, and effect normalization. It intentionally has no dependency on WGPU,
FFmpeg, Clap, the root application, or terminal output.

`video-editor-media` is an internal crate (`publish = false`). It owns FFmpeg,
FFprobe, media probing, `FrameSink`, `FfmpegSink`, temporary output, final
publication, and media errors. It depends on `video-editor-render` with default
features disabled, so media-only work does not enable CPU or WGPU rendering.

The root package remains responsible for application orchestration, CLI
presentation, and the environment preflight part of project loading. Preflight
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

`video-editor-render` is an internal crate (`publish = false`) that owns the
CPU and WGPU renderers, decoded visual assets, renderer geometry and sampling,
effect execution, backend discovery, GPU resources, readback, metrics, and the
staged backend lifecycle. It depends only on `video-editor-core` plus rendering
libraries. Its default features enable `cpu` and `wgpu`; the guaranteed reduced
configuration is `--no-default-features --features cpu`.

Completed frames contain an owned `Vec<u8>` of RGBA pixels. A completion never
exposes a mapped WGPU buffer or reusable readback storage, so its pixels remain
valid after later submissions and polls. WGPU may complete frames out of order;
the root pipeline preserves encoder order using frame numbers.

The root's `src/render/mod.rs` and `src/media/mod.rs` are temporary
compatibility facades. The root application evaluates plans, handles
cancellation, drains and orders completed frames, writes each frame through
`FrameSink`, reports progress, and decides publication. `FrameSink::abort()`
returns structured cleanup errors without replacing the primary render error.
The sink closes stdin, terminates and reaps FFmpeg on abort or active drop, and
joins stderr collection. Root cleanup removes temporary output. Only a
successful `finish()` result that reports the expected frame count is published,
and existing output is rejected unless overwrite was selected.

Frame flow:

```text
core EvaluatedFrame -> renderer submission -> renderer CompletedFrame
-> root completion ordering -> FrameSink -> temporary encoded output -> publication
```

Phase 3 finalization is complete. Phase 4 starts by creating the public `video-editor` SDK
facade and moving root application workflows behind it, then separating the
CLI crate.

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
