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

Core must never depend in the reverse direction. The remaining Phase 1 work is
to move the render-plan compiler and evaluator behind this boundary while
retaining root preflight adapters for filesystem, image, audio, FFprobe,
FFmpeg, and backend checks.

## Migration map

| Current module | Core ownership | Transitional root responsibility |
| --- | --- | --- |
| `diagnostic`, `domain`, `timeline`, `animation` | Canonical implementation | Compatibility re-exports |
| `project/model`, pure `project/validation` | Canonical schema and semantic rules | Filesystem/media/backend preflight and validated-resource handles |
| `plan/compiler/time`, `plan/compiler/tracks`, `plan/schedule` | Canonical deterministic conversion, track normalization, and event construction | Compatibility adapters for existing plan types |
| `plan/model`, remaining compiler modules, `plan/evaluation` | Next extraction unit | Existing renderer-visible plan representation and resource-path attachment |

The plan migration must introduce narrow constructors and accessors for core
types. It must not convert all existing `pub(crate)` fields to unrestricted
public fields merely to cross the crate boundary.

The final target remains five crates:

```text
video-editor-core
video-editor-render
video-editor-media
video-editor
video-editor-cli
```
