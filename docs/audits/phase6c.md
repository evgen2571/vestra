# Phase 6C audit

Recorded on 2026-07-29 in the workspace CI environment.

## Verification executed

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | Workspace formatting gate. |
| `cargo check --workspace` | passed | All five workspace crates compile. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed | No warning suppression added for Phase 6C. |
| `cargo test --workspace -q` | passed | SDK unit tests (51), public SDK tests (21), public-export compile test (1), CLI unit tests (4), plus workspace integration/unit suites. |
| `cargo test -p video-editor-render -q` | passed | 119 renderer CPU, lifecycle, shader, readback, and adapter-gated tests. |
| `cargo test -p video-editor` | passed | 51 SDK unit tests. |
| `cargo test -p video-editor --test public_sdk` | passed | 21 public API tests. |
| `cargo test -p video-editor --test public_exports` | passed | External-style supported-SDK import boundary. |
| `cargo check -p video-editor --no-default-features --features cpu` | passed | CPU-only SDK build. |
| `cargo check -p video-editor-render --no-default-features --features wgpu` | passed with existing dead-code warnings | WGPU-only renderer compilation. |
| `./scripts/verify-public-asset.sh` | passed | Public fixture asset integrity. |
| `python3 crates/video-editor-cli/tests/schema_validation.py` | passed | Project schema compatibility. |
| `./scripts/check.sh` | passed | Repository check script, including CLI integration coverage. |

## Phase 6C evidence

### Diagnostic preservation

The single-frame SDK path now passes the original `Diagnostic` from submit,
poll, validation, flush, and idle failures into `RenderError`; it no longer
reconstructs a generic render diagnostic from code and message. The deterministic
`frame_failures_preserve_complete_backend_diagnostics` test injects
`WGPU-DEVICE-LOST` and verifies the backend category, message, pointer, hint,
and related identifier before confirming prepared-state invalidation.

### Readback ownership, layouts, and callbacks

Mapped WGPU bytes are copied into an independent tightly packed RGBA8 vector,
then unmapped before the slot becomes reusable. `repack_rows()` uses checked
size conversion, multiplication, and row-offset arithmetic. Direct production
tests cover unpadded layouts, padding removal, alignment-boundary widths,
short source/destination buffers, invalid stride, zero dimensions, and checked
layout multiplication overflow.

The callback processor now also validates that a queued callback belongs to the
slot whose queue is being drained. Adapter-gated callback tests invoke
`ReadbackRing::process_callbacks()` with a stale generation after actual slot
reuse, duplicate callback, wrong frame, wrong slot, and callback after abort;
the slot state machine separately proves checked generation overflow. A callback
is accepted only when slot index, generation, frame identity, and state match.

### Public WGPU tests and parity

Public SDK tests exercise `Editor::prepare` and `PreparedProject` for
non-monotonic frame access, owned frame bytes after dropping preparation,
frame → video → frame → video reuse, operation-local video counters, exact
CPU/WGPU metadata/pixel parity for a solid-background fixture, and tolerance-2
public parity checks for the RGBA image/alpha fixture plus first, middle, and
final valid frames of the animation/effects, blend-mode/overlapping
transparency, crossfade, flash, color-adjustment, and Gaussian-blur fixtures.
The public comparison reports maximum per-channel and mean absolute difference
plus the number of channels outside tolerance on failure.

These tests were invoked but skipped their runtime body in this environment:
`WGPU adapter request returned no compatible adapter`. Consequently:

```text
WGPU code compiled.
Pure readback and lifecycle tests passed.
Adapter-gated public WGPU tests were invoked and skipped because no compatible
adapter was available.
Real WGPU single-frame, cross-reuse, and CPU/WGPU runtime parity were not verified.
```

No adapter metadata exists to record. Runtime parity statistics are therefore
not available; the public background fixture requires exact equality when it
runs.

### API and thread safety

`Frame`, `FrameRate`, `PreparationReport`, and `Editor` have compile-time
`Send + Sync` assertions. `PreparedProject` has a `Send` assertion and is
documented as intentionally not `Sync`; render methods require `&mut self`.
`AdapterMetadata` remains the SDK-owned report DTO. The renderer-oriented
`AdapterPerformanceClass` and `PreparationStats` re-exports are deprecated
compatibility exports; new public APIs use SDK report types. The
`public_exports` integration test imports every supported high-level SDK type
from outside the library crate. The README classifies every root export as a
stable high-level API, stable SDK-owned DTO, legacy compatibility export, or
non-exported internal implementation type.

## Audit conclusion

This audit is intentionally not a claim of complete real-adapter verification.
The environment provided no compatible WGPU adapter, so all adapter-gated
runtime coverage is reported as skipped rather than passed.
