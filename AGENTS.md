# Vestra

Rust video editing/rendering engine with core planning/evaluation, rendering
backends, FFmpeg I/O, SDK/CLI surfaces, and Python bindings.

## Architecture
- Preserve separation between project model, plan compilation/evaluation,
  renderers, media I/O, public APIs, CLI, and bindings.
- Core semantics should remain renderer-independent.
- CPU and GPU backends should implement the same supported semantics.
- FFmpeg/codec/process details belong behind the media-I/O boundary.
- CLI and Python bindings should orchestrate engine APIs, not duplicate engine logic.

Use CodeGraph to confirm current crate/module ownership rather than assuming
roadmap or historical structure is still current.

## Invariants
- Treat time/frame/sample conversions and boundary behavior as correctness-sensitive.
- Public Rust APIs, Python APIs, serialized formats, CLI contracts, and report
  schemas are compatibility-sensitive.
- If Rust API changes affect Python bindings, update and verify both.
- Report the backend actually used; do not present CPU fallback as GPU rendering.
- Avoid avoidable allocation/I/O in hot frame/render loops.

## Verification
- Use `./scripts/check.sh` as the canonical repository validation command.
- Otherwise follow workspace/CI configuration for fmt, check, clippy, and tests.
- Renderer/FFmpeg changes should get an appropriate integration render.
- Python-binding changes require Python-side verification.
- Performance claims require comparable before/after measurements.
