# Phase 7A-0: Rust SDK binding readiness

Recorded on 2026-07-29 from the current repository source. This is a Rust SDK
readiness pass only; it does not add a Python package, PyO3, or Python-facing
types.

## Repository state

The workspace members are `video-editor-core`, `video-editor-render`,
`video-editor-media`, `video-editor`, and `video-editor-cli`. The supported
dependency boundary remains:

```text
video-editor-python → video-editor → core / render / media
```

The public SDK boundary is `crates/video-editor/src/lib.rs`. Its default
features are `cpu` and `wgpu`; no renderer, media, WGPU, FFmpeg, render-plan,
or scheduling implementation type is re-exported. The environment supplied
FFmpeg for CPU video tests. It has no compatible WGPU adapter, so adapter-gated
tests retain their existing explicit skip policy and no real WGPU rendering is
claimed here.

## Public API changes

| Old state | New state | Compatibility and coverage |
| --- | --- | --- |
| Inspection field DTOs were public inside a private `application::result` module. | `InspectOutput`, `InspectAssets`, and `InspectAudio` are root exports. | Additive; `tests/public_exports.rs` proves an external crate can name every nested field type. |
| `EditorError` required direct variant matching for category and retained data. | `EditorErrorKind`, `kind`, `timings`, `render_failure_context`, `temporary_output_removed`, and `is_cancelled`. | Additive; `editor_error_accessors_cover_every_variant` covers Project, Plan, and Render. |
| Video observers returned `()`. | Existing `render` and `render_video` stay infallible. `render_with_observer` and `render_video_with_observer` accept `FnMut(RenderEvent) -> RenderObserverControl`. | Additive; cancellation behavior is covered by public SDK tests. |
| `PreparedVideoRenderRequest` required field-inaccessible inspection. `RenderRequest` was field-only. | Immutable request accessors for output/overwrite and output/overwrite/preview/backend respectively. | Additive. |
| Several categorical SDK types had only serde-derived names. | Stable `as_str` methods now cover `Category`, `Severity`, backend preference/kind, pixel format, timing scope, failure stage, and the pre-existing adapter types. | Existing JSON rename rules are unchanged; `stable_sdk_enum_strings_match_report_names` locks representative names. |

`public_exports.rs` is now an exhaustive compile-time import contract for the
intended root surface, including errors, validation/version results, inspection
DTOs, failure DTOs, categorical types, and observer control. No internal type
was made public to enlarge this test.

## Error mapping readiness

The current error variants are:

| Variant | Stable inspection |
| --- | --- |
| `Project` | kind, diagnostics, warnings, timings; no render context or temporary output exists (`None`). |
| `Plan` | kind, single diagnostic slice, warnings, timings; no render context or temporary output exists (`None`). |
| `Render` | kind, diagnostics, warnings, timings, `RenderFailureContext`, and temporary-output cleanup status. |

Cancellation is structurally detectable by `EditorError::is_cancelled()` and
the diagnostic category `Category::Cancellation`; its error kind remains
`Render`. Existing `Display`, `Error`, diagnostic structure, and report serde
shapes were not changed.

## Inspection/report readiness

`InspectionReport` has SDK-owned, root-nameable `output`, `assets`, and
optional `audio` DTOs. The integration test names each type and accesses the
corresponding report field without importing a private module. Reports remain
immutable snapshots; this work adds no mutable report API.

## Trait and ownership matrix

| Type | Send | Sync | Mutable operation required | Binding implication |
| --- | ---: | ---: | ---: | --- |
| `Editor` | yes | yes | no | Shareable entry point. |
| `Project` | yes | yes | no | Immutable project input can cross threads. |
| `PreparedProject` | yes | intentionally no | yes | Move while idle; serialize access to frame/video operations externally. |
| `Frame` | yes | yes | no | Owned pixels are safe to transfer/share. |
| `FrameRate` | yes | yes | no | Value type. |
| `CancellationToken` | yes | yes | no | Cloneable cooperative cancellation. |
| `PreparationReport` | yes | yes | no | Immutable snapshot. |
| `AdapterInfo` | yes | yes | no | SDK DTO, not a WGPU handle. |
| `RenderPerformance` | yes | yes | no | Immutable result DTO. |
| public requests | yes | yes | no | Plain immutable inputs once passed. |

`public_auto_traits_are_explicit` compile-asserts every positive bound above.
`PreparedProject` is asserted `Send` only. Its concrete private backend is
deliberately not `Sync`, its operations require `&mut self`, and its API docs
state the exclusive-operation contract; this preserves non-shared-concurrent
use without incorrectly claiming a stable negative assertion mechanism.

## Callback and publication audit

### Verified control flow

1. `render_prepared_with_sink` prepares `OutputTarget` at
   `render/engine/runner.rs:424-442`, snapshots metrics, and emits `started` at
   `:458-478` before encoder startup.
2. The encoder starts after `started`; `frame_loop::run` submits frames, polls
   completions, orders them, writes each frame, and emits `progress` only for
   `0 < completed_frames < total_frames`.
3. Cancellation is checked before submission, after cancellable polling, while
   draining, before every ready-frame write, and immediately after every
   forwarded progress callback. The runner also checks a callback-triggered
   token cancellation after `started`, before encoder startup. Pre-submission
   cancellation leaves the prepared backend reusable where supported; after
   submission, cancellation aborts the backend and sink, then removes the
   temporary output through `cleanup_error`.
4. After all frames are written, the runner verifies operation metrics and
   backend idle state, then makes one final cancellation check before encoder
   finalization. A cancellation here is post-submission: it aborts the backend,
   invalidates `PreparedProject`, aborts the operation-local encoder, and
   removes the temporary output. It neither finalizes the encoder nor publishes
   the final output. Later prepared operations return
   `VESTRA-PREPARED-INVALIDATED`. Only a passing check finalizes the encoder and
   renames the temporary output.
5. Only after publication does it emit `completed`. Its observer result is
   ignored because the operation is already terminal.

The event sequence is `started`, zero or more pre-publication `progress`
events, then post-publication `completed`. Every progress event has
`0 < frame < total_frames` and `0.0 < progress < 1.0`. A one-frame successful
render emits `started` then `completed`. Warnings are carried by
`completed.warnings`.

### Prepared-state lifecycle

| Failure/cancellation point | Submission occurred | Reusable | Later SDK result |
| --- | ---: | ---: | --- |
| Before submission | No | Yes, where supported | Normal later operation |
| During or after frame work | Yes | No | `VESTRA-PREPARED-INVALIDATED` |
| Final pre-finalization checkpoint | Yes | No | `VESTRA-PREPARED-INVALIDATED` |
| After publication / `completed` observation | Operation already succeeded | Yes unless another failure exists | Published success |

The final pre-finalization case is deliberately conservative: native frame
submission and backend work have occurred, so backend abort/cleanup is paired
with invalidation even if a CPU backend could otherwise appear reusable. This
keeps CPU and WGPU callers on the same stable SDK diagnostic.

### Can callback-triggered cancellation always be observed before publication?

Yes for pre-publication events. `RenderObserverControl::Cancel` is observed
synchronously after `started` and each progress callback. A legacy `()`
callback can cancel through `CancellationToken`; the runner checks that token
after `started`, while the frame loop checks it after progress and before every
ready-frame write. A final runner check closes the gap between the frame loop
and encoder finalization. The deterministic tests cover cancellation with a
ready queue, cancellation at the final checkpoint through idle verification,
temporary cleanup, no final output, and prepared-state reuse versus
invalidation based on whether submission began. The final-checkpoint regression
then invokes the same prepared state and observes `VESTRA-PREPARED-INVALIDATED`.

`completed` is intentionally post-publication. Its observer result is ignored.
Returning `Cancel` cannot remove an already renamed output, replace success
with cancellation, or invalidate a prepared project. Phase 7C should either
forward only pre-publication events to Python, or forward `completed` with this
exact non-rollback contract.

## Commands and results

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo check --workspace` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` | passed |
| `cargo test -p video-editor` | passed: 58 unit, 2 export, 31 public SDK tests |
| `cargo test -p video-editor --test public_sdk` | passed: 31 tests |
| `cargo test -p video-editor --test public_exports` | passed: 2 tests |
| focused staged lifecycle and observer tests | passed, including final pre-finalization cancellation and subsequent `VESTRA-PREPARED-INVALIDATED` |

## Readiness verdict

**Ready for Phase 7A.** The Rust boundary now supplies stable report/error
inspection and a safe callback-control seam without beginning package or PyO3
work.

Remaining phase work:

- Phase 7A: package design and immutable Python API mapping.
- Phase 7B: prepared execution and frame ownership wrappers.
- Phase 7C: retain/re-raise Python callback exceptions and choose the documented
  post-publication `completed` forwarding policy.
