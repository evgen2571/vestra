# Phase 6A audit

Recorded on 2026-07-29 in the workspace CI environment. This audit was
refreshed after the final timing, metric, ownership, and idle-safety pass.

| Command | Result | Mode and notes |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | Formatting verification. |
| `cargo check --workspace` | passed | Default workspace build. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed | Full workspace lint. |
| `cargo test --workspace` | passed | Workspace unit and integration coverage, including CLI rendering and CPU/WGPU regression tests. |
| `cargo test -p video-editor-core --lib` | passed | 33 core-library tests, including the corrected base-directory path fixture. |
| `cargo clippy -p video-editor --all-targets --all-features -- -D warnings` | passed | SDK crate lint verification. |
| `cargo check -p video-editor --no-default-features --features cpu` | passed | CPU-only SDK compile. |
| `cargo check -p video-editor-render --no-default-features --features wgpu` | passed with existing dead-code warnings | WGPU compile-only verification; no compatible-adapter runtime reuse claim is made. |
| `python3 crates/video-editor-cli/tests/schema_validation.py` | passed | Schema validation. |
| `scripts/verify-public-asset.sh` | passed | Public asset verification. |
| `scripts/check.sh` | passed | Repository compatibility script, including SDK and CLI test coverage. |
| `cargo test -p video-editor --lib` | passed | 46 tests; includes renderer and application-level prepare-once/render-twice, success/failure timing snapshots, metric reset, pre-submission reuse, invalidation, visual snapshot, and injected idle-failure coverage. |

Phase 6A keeps `PreparedState` private to the SDK renderer. It owns the visual
execution snapshot: compiled plan and schedule, decoded images, requested and
selected backend identity, fallback metadata, and backend resources. The private
`PreparedRender` wrapper retains immutable preparation warnings, timing snapshots,
and the small result metadata needed after validation rather than the full validated
project. Each video operation separately owns its output target,
FFmpeg sink, temporary file, ordering buffer, and progress state. Preparation
does not start FFmpeg or require an output path. Audio is still passed to FFmpeg
at operation time, so repeated deterministic video renders require the source
audio file to remain available and unchanged.

The renderer-level deterministic reuse test prepares once, performs two video
operations with two output paths, asserts one backend construction, and checks
that each result contains only that operation's staged frame metrics. The
application-level test prepares a `PreparedRender` once and exercises its own
bridge twice with separate sinks and outputs. Backend preparation timing
is retained in the private state and merged into one-shot SDK results and render
failures; operation timing contains only the current video operation. The plan
is retained through `Arc<RenderPlan>` and result assembly uses a small private
metadata snapshot instead of cloning the resolved project per operation.
Command submissions and cache requests use checked before/after deltas; cache
occupancy remains a persistent after-operation snapshot. The invalidation test
asserts that a submission failure makes later reuse return
`MVP-PREPARED-INVALIDATED`. An already-cancelled operation aborts its sink but
does not abort or invalidate an untouched backend; a fresh operation then reuses
that same backend. WGPU runtime reuse has not been claimed: the
deterministic lifecycle tests use a mock staged backend; WGPU compile and any
available runtime checks are recorded by the final verification sweep.

The visual snapshot test uses the real CPU backend: it prepares from a copied
project, replaces the source image, and verifies the existing prepared state
emits identical RGBA frames while a newly prepared state emits different frames.

`Editor::render` now explicitly invokes private preparation and then a private
prepared video operation. There is no remaining one-shot renderer that builds
its own plan, decoded assets, schedule, and backend independently.
