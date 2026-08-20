# Phase 8 summary

Phase 8A introduced `ProjectBuilder`; 8B added image/audio assets, image and
solid clips, transforms, crop, opacity, and global audio. 8C-A added typed
tracks/keyframes and interpolation. 8C-B added ordered effects and blend
modes. 8C-C added transitions and flashes. 8C-D added native canonical
presets and explicit timeline helpers. 8C-E is the release-candidate
conformance pass.

## Completion definition

Phase 8 complete means complete typed Python authoring coverage for the current
schema-version 1 project model. It provides a stable typed Python authoring SDK,
native validation, CPU rendering, WGPU-capability integration, wheel packaging,
and downstream Python usability. It does not mean the overall video editor is
feature-complete.

Python users can author every current schema-v1 visual and audio feature through
typed public APIs, validate it natively, preflight it, and render it on CPU,
with adapter-gated WGPU support. `Project.from_dict()` is a lower-level
construction path for the same current schema-v1 model, including existing
canonical JSON, integration data, and generated dictionaries. No current
schema-v1 field remains raw-only.

Video assets, multi-track audio and mixer features, nested compositions, and
audio-reactive visual systems are future native project-model and schema work.
Neither `Project.from_dict()` nor `ProjectBuilder` can express them until that
work exists.

Validation is layered: Python protects typed authoring and ownership; native
validation owns semantic/timeline diagnostics; preflight owns runtime media and
backend readiness; rendering owns FFmpeg/output failures. The public graph is
ready for downstream Python applications on the verified CPython 3.13 runtime;
other metadata-supported Python versions were not exercised in this environment.

## Final verdict

Phase 8 complete.

The typed Python authoring API covers the complete current schema-version 1
project model and is ready for downstream use. The strict WGPU selection reports
the expected absence of a compatible adapter in this environment; normal WGPU
tests remain adapter-gated. This does not indicate a schema-v1 authoring gap.
