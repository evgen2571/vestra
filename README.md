# video-editor

`video-editor` is a standalone JSON-driven declarative video renderer written in Rust. It accepts one typed-track JSON project format, composites a deterministic timeline of image and full-canvas solid-colour layers, optionally places one audio track, and writes a playable MP4 without interactive input.

It requires Rust 1.85+ to build and FFmpeg/FFprobe 7+ at runtime. The supported output is H.264 MP4 with `yuv420p` video and AAC audio. Image inputs use formats supported by the Rust `image` crate (including PNG, JPEG, GIF, WebP, BMP, TIFF, and QOI); audio inputs are probed and decoded by FFmpeg (WAV and MP3 are practical baseline formats).

```bash
cargo build --release -p video-editor-cli

# Run these from the repository root.
./target/release/video-editor validate examples/projects/animation-effects.json
./target/release/video-editor inspect examples/projects/animation-effects.json --format json
./target/release/video-editor render examples/projects/animation-effects.json --progress json
```

The reference output is written under `examples/output/` and is protected from accidental replacement. Re-render it only with `--overwrite`.

Key commands are non-interactive:

```text
video-editor validate <project> [--format human|json]
video-editor inspect <project> [--preview] [--format human|json]
video-editor render <project> [--output PATH] [--overwrite] [--preview]
                    [--render-backend auto|cpu|wgpu]
                    [--format human|json] [--progress human|json|none] [--report PATH]
video-editor version
```

`--format json` writes a result envelope. `render --progress json` writes JSON Lines events, starting at zero and ending with `completed` at 1.0 only after publication. Exit statuses are 0 (success), 1 (internal), 2 (usage), 3 (project), 4 (asset/media), 5 (backend/render), 6 (output), and 130 (interrupt cancellation).

Rendering backend selection is a runtime option and is never stored in project
JSON. `cpu` always uses the deterministic CPU compositor. `wgpu` requires a
headless compatible adapter and fails without falling back. `auto` attempts
WGPU during preparation and falls back to CPU only before the first frame;
reports expose `requested_render_backend`, selected `render_backend`, optional
`backend_fallback`, and adapter metadata. WGPU is headless: decoded image
textures, pipelines, and readback storage persist for a render; each output
frame is still read back to CPU RGBA for FFmpeg, so it is not zero-copy or
hardware video encoding.
Set `VIDEO_EDITOR_WGPU_FORCE_FALLBACK=1` to prefer a fallback adapter, or
`VIDEO_EDITOR_WGPU_BACKEND=vulkan|gl|metal|dx12` to constrain adapter discovery
for CI or headless troubleshooting.

See [the project format](docs/specs/project-format.md), [the effects-ready example](examples/projects/effects-ready-v1.json), [the effects benchmark recipe](docs/benchmarks/effects-ready-v1.md), [the WGPU renderer guide](docs/wgpu-renderer.md), [the canonical schema](schemas/project.schema.json), and [the animation/effects example](examples/projects/animation-effects.json) for the complete contract. Run the canonical check suite with:

```bash
python3 -m pip install -r requirements-dev.txt
./scripts/check.sh
cargo bench -p video-editor --bench animation_effects
./scripts/render-effect-examples.sh
```

The benchmark uses the canonical animation/effects fixture at 720×1280. Run it
with `VIDEO_EDITOR_BENCH_BACKEND=cpu` or `VIDEO_EDITOR_BENCH_BACKEND=wgpu` to
select the renderer; it performs five warmups and reports median/range values
across five measured renders, along with the selected backend and available GPU
preparation timings. WGPU benchmark results require a compatible adapter.

Focused effect, transition, preset, and compositing projects render at least
90 frames. The helper writes predictable CPU preview files below
`examples/output/`; pass `--skip-existing` only to retain existing files.

`requirements-dev.txt` pins the Python package used by the JSON Schema check;
Rust dependencies are locked in `Cargo.lock`.

## Rust SDK

The workspace root is virtual. `video-editor` is the supported Rust SDK and
`video-editor-cli` supplies the `video-editor` executable. A normal Rust
consumer needs only `video-editor`:

```rust
use video_editor::{BackendPreference, CancellationToken, Editor, RenderRequest};

let editor = Editor::new();
let project = editor.load_project("project.json")?;
let result = editor.render(
    &project,
    RenderRequest { output: Some("result.mp4".into()), overwrite: true,
        preview: false, backend: BackendPreference::Auto },
    &mut |_| {},
    &CancellationToken::new(),
)?;
```

The SDK exposes structured inspection, validation, preflight, render results,
events, and errors. A successful video operation emits `started`, zero or more
pre-publication `progress` events with values below 1.0, then `completed` only
after the output is published. `RenderObserverControl::Cancel` can stop a
pre-publication operation. A `Cancel` returned for `completed` is ignored
because publication has already succeeded. The SDK neither initializes logging
nor prints or exits. CLI formatting, Ctrl-C installation, and exit-code mapping belong to
`video-editor-cli`. `video-editor-core`, `video-editor-render`, and
`video-editor-media` are implementation crates. Their public items support
the workspace and are not stable SDK contracts.

### Project paths and lifecycle

`Project::load("/a/b/project.json")` uses `/a/b` as its base directory.
`Project::from_json(json, "/a/b")` and `Project::from_value(value, "/a/b")`
use the supplied directory. During preflight and rendering, every canonical
relative asset and output path resolves against that base directory. Absolute
paths stay absolute. Serializing or saving a `Project` preserves the canonical
path strings; it does not rewrite relative paths to absolute paths.

The lifecycle is project parsing, pure validation, operation-aware preflight,
plan compilation, render preparation, staged rendering, ordered `FrameSink`
delivery, then temporary output publication. `Editor::validate` is deterministic
and never opens assets or starts a subprocess. `Editor::preflight` checks only
the environment required by its explicit target. The CLI `validate` command uses
the complete default render-readiness target, including assets, required media
probing, encoder, backend, and configured output. `Editor::prepare` performs
preparation-target preflight without checking an output path or starting FFmpeg.
It returns an owned `PreparedProject` that can outlive both the `Editor` and
`Project`. The snapshot freezes visual assets and resolved metadata. FFmpeg
reopens external media, including audio, per video operation, so those files
must remain unchanged for repeatable output.

```rust
use std::time::Duration;
use video_editor::{BackendPreference, Editor, PrepareOptions};

let project = Editor::new().load_project("project.json")?;
let mut prepared = Editor::new()
    .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))?;
let frame = prepared.render_frame(Duration::from_secs(2))?;
assert_eq!(frame.as_bytes().len(), frame.width() as usize * frame.height() as usize * 4);
```

Prepared video operations use `PreparedVideoRenderRequest`; the backend cannot
change after preparation. CPU and WGPU snapshots support synchronous single-frame
rendering through the same staged submit/completion/flush lifecycle. WGPU frame
readback remains internal: it removes GPU copy-row padding and copies pixels into
an independent CPU-owned RGBA8 allocation before its ring slot is reusable. Frame
pixels are owned RGBA8, top-row-first, tightly packed, and unpremultiplied.
The mapped GPU buffer is unmapped before a readback slot becomes reusable; returned
`Frame` bytes never borrow GPU buffers or readback slots. A WGPU callback is
accepted only when its slot index, checked slot generation, frame identity, and
slot state all match. A mismatched callback or device-loss diagnostic invalidates
the prepared backend rather than falling back or recreating a device. Backend,
media, and core diagnostics retain their structured category, code, severity,
pointer, hint, and related identifier at the SDK error boundary.
`Frame::timestamp()` is the earliest `Duration` that maps back to that frame.
Timeline conversion uses checked integer rational arithmetic; when a fractional
frame boundary lies between nanoseconds, it is rounded up to the next
nanosecond. The final project duration is an exclusive endpoint. Random frame
access uses the same canonical draw ordering and selected staged backend path as
video rendering. Prepared-video timing totals exclude reusable preparation;
one-shot totals include it. Preparation warnings are merged in lifecycle order
and deduplicated by their complete diagnostic identity.

`Frame`, `FrameRate`, `PreparationReport`, `AdapterInfo`, `RenderPerformance`,
and `Editor` are `Send + Sync`.
`PreparedProject` is `Send` but intentionally not `Sync`: it can move between
threads while idle, and every render method requires `&mut self`; video progress
callbacks run on the calling thread. The supported SDK surface is the API exported
by `video-editor`; renderer crates and renderer-oriented implementation DTOs are
workspace internals rather than contracts for future bindings.

## Python prepared frames

The Python package wraps the public Rust SDK. Prepare once, then request owned
RGBA8 frames by frame number or exact integer nanoseconds:

```python
import video_editor

project = video_editor.Project.load("project.json")
prepared = video_editor.Editor().prepare(
    project,
    video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
)
frame = prepared.render_frame_number(0)
assert frame.width == prepared.preparation_report.width
pixels = frame.to_bytes()
```

`render_frame_ns()` is the exact timestamp path. `render_frame_seconds()` is a
float convenience method that truncates to nanoseconds, so it is not exact.
The final project duration is exclusive. A prepared object permits one operation
at a time; a concurrent call raises `PreparedProjectBusyError` immediately.
Frames own their Rust pixel allocation, and `to_bytes()` copies it into Python
bytes. The package does not expose a buffer protocol, NumPy arrays, Pillow
images, or GPU memory.

### API stability

The public exports are classified as follows:

- Stable high-level SDK: `Editor`, `EditorBuilder`, `EditorError`, `EditorErrorKind`, `Project`,
  `PrepareOptions`, `PreparedProject`, `PreparedVideoRenderRequest`,
  `RenderRequest`, `CancellationToken`, `RenderEvent`, `RenderObserverControl`, `RenderResult`, and
  `RenderTimingScope`.
- Stable SDK-owned DTOs: `Frame`, `PixelFormat`, `FrameRate`,
  `FrameRateError`, `PreparationReport`, `PreparationTimings`,
  `ValidationReport`, `PreflightReport`, `InspectionReport`, `InspectOutput`,
  `InspectAssets`, `InspectAudio`, `VersionResult`,
  `Diagnostic`, `Category`, `Severity`, `BackendPreference`, `BackendKind`,
  `BackendFallback`, `AdapterInfo`, `AdapterDeviceType`, `GraphicsBackend`,
  `RenderPerformance`, `RenderFailureContext`,
  `RenderFailureStage`, and `RenderTimings`.
- Internal leaks: none. Renderer crate types, render plans, WGPU resources,
  and staged backend interfaces are not public SDK contracts.

`Project::load` and `Project::from_json` record JSON parse time. `from_value`
does not parse JSON and reports zero parse time. Render reports separate that
earlier parse time from `operation_total_ms`, which measures only work inside
`Editor::render`. Successful CLI render output serializes that SDK result and
does not add a command-total field. Selected CLI failure reports include an
`elapsed_ms` measured by the command, which can include project loading.

`AdapterInfo` is the stable SDK-owned adapter report. `RenderPerformance` is
also SDK-owned, but it is a deliberately curated report rather than a mirror of
renderer state: its serialized fields preserve the existing JSON/report schema.
They cover operation facts, compilation/asset/cache observations, and stable
resource counts for one CPU or WGPU video operation. Repeated prepared-video
operations report their own deltas; one-shot reports include preparation in its
timing scope, while prepared operations do not. Readback-ring configuration,
slot and callback lifecycle data, polling counters, staging estimates, and
fine-grained readback timings are Rust-only advanced diagnostics and are
excluded from JSON. Python v0.1 will consume only SDK-owned DTOs and a
deliberately selected subset of these metrics.

Create a source package from tracked files only. This omits ignored render
outputs, reports, temporary files, benchmark output, and Cargo build artifacts.

```bash
git archive --format=zip --output=video-editor.zip HEAD
```

## Python package development

Phase 7A provides immutable Python bindings for project loading, conversion,
validation, preflight, inspection, diagnostics, and reports. Phase 7B adds
preparation, `PreparedProject`, `PreparationReport`, exact frame timing, and
synchronous CPU or WGPU single-frame rendering with copied frame bytes. A
prepared object permits one operation at a time.

Phase 7C is still deferred. It will cover video rendering, progress events,
cancellation, callback exceptions, render results, and performance reports.

```bash
python3 -m venv .venv
.venv/bin/python -m pip install maturin pytest mypy
.venv/bin/maturin develop
.venv/bin/python -m pytest python-tests
```

Use `import video_editor`. `Project.from_dict()` follows the same native path
as JSON, and package path properties return `pathlib.Path` values. The full
binding contract is recorded in `docs/audits/phase7a.md`.

The currently exposed Python API is immutable. `Project.from_dict()` accepts
`collections.abc.Mapping` values. Python does not yet expose video rendering,
callbacks, cancellation, or asynchronous rendering.
