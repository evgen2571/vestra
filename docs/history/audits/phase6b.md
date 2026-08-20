# Phase 6B audit

Recorded on 2026-07-29 in the workspace CI environment.

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | Formatting gate. |
| `cargo check --workspace` | passed | Default workspace build. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed | No broad warning suppression. |
| `cargo test --workspace` | passed | Full workspace suite. |
| `cargo test -p video-editor --lib` | passed | 49 tests: staged lifecycle, fallback, invalidation, reuse, CPU pre-encoding parity, and WGPU frame deferral. |
| `cargo test -p video-editor --test public_sdk` | passed | 16 tests: public ownership, timestamp, timing, auto-traits, and compatibility coverage. |
| `cargo check -p video-editor --no-default-features --features cpu` | passed | CPU-only SDK compilation. |
| `cargo check -p video-editor-render --no-default-features --features wgpu` | passed | WGPU compile-only verification. |
| `python3 crates/video-editor-cli/tests/schema_validation.py` | passed | Schema compatibility. |
| `scripts/verify-public-asset.sh` | passed | Public asset check. |
| `scripts/check.sh` | passed | Repository compatibility script. |

The timeline tests exercise 24/1, 30/1, 60/1, 24000/1001, 30000/1001, and
60000/1001 across 10,000 frame round trips. Public SDK coverage verifies the
30000/1001 frame-17 ceiling boundary, exclusive final duration, owned-frame
reuse and post-drop retention, report immutability, and prepared-operation
timing aliases. `random_cpu_frame_matches_the_pre_encoding_video_frame` captures
the normal video path at `FrameSink` and compares its pre-encoding pixels with
the random CPU frame byte-for-byte. Sequential and random frame paths both use
the core draw-order helper. WGPU is compiled by the all-features workspace checks; a real WGPU
single-frame runtime was intentionally not attempted because Phase 6B keeps it
unsupported.

`random_cpu_frames_match_every_pre_encoding_animation_effects_frame` extends
that comparison across every frame of the animation/effects fixture.

`unsupported_wgpu_frame_request_is_non_destructive` uses the deterministic WGPU
backend seam to prove a frame request submits no work, returns the structured
capability error, and leaves prepared-video rendering usable.

Compile-time assertions verify `Frame` and `Editor` are `Send + Sync`.
`PreparedProject` intentionally makes no `Send`/`Sync` guarantee in Phase 6B:
operations require `&mut self` and it owns an opaque backend trait object.
