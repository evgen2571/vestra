# Phase 8 summary

Phase 8A introduced `ProjectBuilder`; 8B added image/audio assets, image and
solid clips, transforms, crop, opacity, and global audio. 8C-A added typed
tracks/keyframes and interpolation. 8C-B added ordered effects and blend
modes. 8C-C added transitions and flashes. 8C-D added native canonical
presets and explicit timeline helpers. 8C-E is the release-candidate
conformance pass.

Python users can now author every Phase-8 schema-v1 visual and audio feature
through typed public APIs, validate it natively, preflight it, and render it on
CPU (with adapter-gated WGPU support). Raw `Project.from_dict()` remains for
intentionally deferred schema evolution and non-Phase-8 capabilities: video
assets, multiple audio tracks/mixing, nested compositions, and editor UI.

Validation is layered: Python protects typed authoring and ownership; native
validation owns semantic/timeline diagnostics; preflight owns runtime media and
backend readiness; rendering owns FFmpeg/output failures. The public graph is
ready for final architecture review. It is suitable for downstream Python
applications on the verified CPython 3.13 runtime; other metadata-supported
Python versions were not exercised in this environment.
