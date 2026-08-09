# video-editor

`video-editor` is a standalone JSON-driven declarative video renderer written in Rust. It accepts one typed-track JSON project format, composites a deterministic timeline of image and full-canvas solid-colour layers, and can author an ordered audio timeline of mixer tracks and clips.

## Python authoring

The Python package has mutable pure-Python authoring alongside immutable native
project, execution, and report APIs. `ProjectBuilder` creates an owned canonical
schema-version 2 dictionary, then `build()` passes that dictionary to
`Project.from_dict()` and returns the existing immutable native `Project`.
There is no second project format.

Phase 9 executes typed schema-version 2 audio mixer timelines. The old
single global audio placement and schema version 1 are intentionally rejected;
there is no automatic migration.

```python
from video_editor import Editor, FrameRate, RenderRequest
from video_editor.authoring import Interpolation, Point, ProjectBuilder, Sizing

builder = ProjectBuilder(
    width=160,
    height=90,
    frame_rate=FrameRate(30, 1),
    output_path="canonical-output.mp4",
    duration=None,
    output_audio=True,
    background="#101018",
)

cover = builder.add_image_asset("examples/assets/red.png")
music = builder.add_audio_asset("examples/assets/tone.wav")
clip = builder.add_image_clip(
    source=cover, start=0.0, duration=1.0, layer=0, sizing=Sizing.cover(),
)
clip.transform.position.base_value = Point(0.5, 0.5)
clip.opacity.keyframe(time=0.0, value=0.0)
clip.opacity.keyframe(time=0.5, value=1.0, interpolation=Interpolation.EASE_OUT)
track = builder.audio.add_track(id="music")
track.add_clip(asset=music, start=0.0)

data = builder.to_dict()
project = builder.build()
validation = builder.validate()
assert validation.is_valid

result = Editor().render(project, RenderRequest("result.mp4", overwrite=True))
print(result.output_path)
```

`output_path` is required because it remains part of the project contract,
serialization, and default runtime behavior even when `RenderRequest` supplies
a different destination. `base_directory` is runtime context, never appears in
`to_dict()`, and is passed to every native build. The builder copies metadata at
construction and returns a fresh deep snapshot on every `to_dict()` call. A
top-level `metadata=None` omits the field. Nested `None` values inside supplied
metadata become JSON null.

Image and audio asset IDs share one namespace. Generated IDs use `image-`,
`audio-`, and `clip-` prefixes with fixed-width counters. Asset paths remain
relative until native preflight resolves them against `base_directory`; adding
an asset never checks the filesystem. Image clips require a static transform,
while solid-colour clips are full-canvas and deliberately have no sizing, crop,
or transform properties. Assets, clips, tracks, transforms, and audio tracks
are created only by builder factories. Their stable owned objects cannot be
replaced; mutate a track's `base_value` instead. Static transform, crop, and
opacity values serialize as constant `{"base_value": ...}` tracks until you add a
keyframe. `track.keyframe(time=..., value=..., interpolation=...)` works on
scalar, point, and crop tracks. Keyframes are immutable values, returned in an
immutable tuple, and their clip-local seconds stay in authored order. Native
validation reports duplicate, unordered, and out-of-clip times. Before the
first keyframe a track uses `base_value`; each keyframe's interpolation controls
the segment ending at it, and the final keyframe holds. Use
`Interpolation.LINEAR`, `HOLD`, `EASE_IN`, `EASE_OUT`, or `EASE_IN_OUT`, or
`CubicBezier(x1, y1, x2, y2)`. Bézier x controls must be in `0..=1`; y controls
are finite and unrestricted. `clear_keyframes()` keeps the stable track and
its base value. A disabled crop retains its animated state and serializes again
when `set_crop()` re-enables it.

Clip effects and global post-effects are ordered builder-owned collections.
Every numeric effect parameter uses the same `ScalarTrack` contract as opacity:
mutate `base_value` or append keyframes. Ordinary clip-effect tracks use
clip-relative time. Camera-shake tracks use time relative to the effect's active
interval start. Global post-effect tracks use project-relative time. Effect IDs
are unique within a clip or within `builder.post_effects`, not across those
scopes. Obtain collections only through `clip.effects` and
`builder.post_effects`. The order in `items` is the rendering and canonical
serialization order.

```python
from video_editor.authoring import BlendMode, Interpolation

clip.blend_mode = BlendMode.SCREEN
brightness = clip.effects.add_brightness(amount=0.0)
brightness.amount.keyframe(time=0.0, value=0.0)
brightness.amount.keyframe(time=0.5, value=0.3, interpolation=Interpolation.EASE_OUT)
builder.post_effects.add_vignette(amount=0.25, radius=0.8, softness=0.3, colour="#000000")
```

Coordinated transitions and flashes are project-level collections. Transitions
require two owned `ImageClip` objects. The builder checks handle ownership and
type immediately; native validation reports timeline fit, hidden clips, and
overlapping transition associations after later clip edits.

```python
incoming = builder.add_image_clip(
    source=cover, start=0.5, duration=1.0, layer=1, sizing=Sizing.cover(),
)
builder.flashes.add(
    start=1.2, duration=0.08, colour="#ffffff", opacity=0.8, fade_out=0.08,
)

# Presets are canonical nodes. Native compilation supplies their animation.
clip.presets.apply_impact(seed=17, intensity=0.8)

# `incoming` moves from [0.50, 1.50] to [0.75, 1.75], leaving exactly
# 0.25 seconds of overlap with `clip`, which spans [0.00, 1.00].
builder.timeline.shift_clip(incoming, delta=0.25)
builder.timeline.add_crossfade_between(clip, incoming, duration=0.25)
```

`builder.timeline.shift_clip(...)` changes `clip.start` only. It does not move
existing transitions, standalone flashes, global post-effect keyframes, or
other project-relative events. Clip-local transform, opacity, crop, and
clip-effect keyframes, plus preset-local timing, stay unchanged, so those
visual behaviors move with the clip. Shift a clip before creating a related
transition. If a later shift makes an existing transition invalid,
`builder.validate()` reports the native validation problem. This helper is
deliberately narrow. It does not repair a timeline.

`builder.timeline.add_crossfade_between(...)` never moves or trims clips. It
uses the beginning of their existing overlap and requires that overlap to be at
least the requested duration.

The supported blend modes are `normal`, `add`, `screen`, `multiply`, and
`overlay`. All ordinary effects are available on clip and post-effect
collections; `camera_shake` and `motion_blur` are clip-only because they depend
on a clip's local transform and motion. `ActiveInterval(start, duration)` is a
finite half-open clip-local interval for camera shake. Python rejects malformed
local types and non-finite values; native validation remains responsible for
semantic ranges, timeline fitting, and preparation capability.

Every add operation validates before reserving an ID, so a failed call leaves
the builder unchanged and does not consume an explicit or generated ID. An
image clip always owns one crop track: `set_crop()` updates and enables it,
and `clear_crop()` disables serialization without detaching retained handles.

Schema version 2 uses a stable `builder.audio` timeline. Add ordered mixer
tracks with `builder.audio.add_track()` and ordered placements with
`track.add_clip()`. `output_audio` is an independent constructor/property
switch; it controls mux eligibility and does not change authored timeline
duration or validation. With `duration`, the builder emits `duration_mode:
"explicit"`; without one it emits `"automatic"`, which native preflight
resolves from visual and authored audio content. A clip may also have
clip-local gain automation with immutable `AudioGainKeyframe` values. Audio
uses source-keyframe interpolation: each keyframe's interpolation controls the
segment from that keyframe to the following keyframe. This differs from visual
animation tracks, whose keyframe interpolation controls the segment ending at
that keyframe. The last audio gain keyframe's interpolation has no effect,
because its gain holds through the selected clip end. Static clip gain
multiplies the automation and fade envelopes. `builder.audio.crossfade(outgoing, incoming,
curve=AudioFadeCurve.EQUAL_POWER)` configures matching fades over an existing,
authoring-resolvable overlap; it never moves or trims either clip. Mute, gain, and output muxing
do not change structural duration. `build()` parses only.
`validate()` invokes deterministic native validation and keeps native
diagnostics intact. Preflight and rendering remain separate operations and
require their normal runtime dependencies.

```python
from video_editor.authoring import (
    AudioFadeCurve, AudioGainInterpolation, AudioGainKeyframe,
)

music = builder.add_audio_asset("music.wav")
outgoing = builder.audio.add_track(id="outgoing").add_clip(
    asset=music, start=0.0, trim_end=4.0,
)
incoming = builder.audio.add_track(id="incoming").add_clip(
    asset=music, start=3.0, trim_end=5.0,
)
outgoing.set_gain_automation([
    AudioGainKeyframe(0.0, 0.0, AudioGainInterpolation.LINEAR),
    AudioGainKeyframe(0.5, 1.0, AudioGainInterpolation.HOLD),
    AudioGainKeyframe(1.0, 0.8),
])
builder.audio.crossfade(outgoing, incoming, curve=AudioFadeCurve.EQUAL_POWER)
```

This uses the existing one-second overlap. The helper changes only fade
durations and curves. It does not move either clip or rewrite source trims.

### Audio capabilities and limits

The audio timeline supports multiple tracks, multiple overlapping clips, source
trims, sample-accurate placement at the 48 kHz mixer resolution, static clip
and track gain, clip gain automation with linear or hold interpolation, linear
and equal-power fades, and an explicit crossfade helper. FFmpeg mixes the
declared graph deterministically. It does not normalize automatically and does
not apply an automatic limiter.

The authored limits are 256 audio tracks, 4,096 audio clips, and 16,384 gain
automation keyframes. A render may open 128 unique audible source paths after
deduplication. That is an execution limit, not an authored graph limit.

Track gain automation, pan, EQ, compression, reverb, ducking, time stretching,
pitch shifting, beat detection, and BPM analysis are not implemented. The mixer
is a timeline mixer, not a mastering chain.

### Audio-reactive scalar properties

Visual scalar properties can react to the final mixed Master waveform. Signals
are immutable values, so one chain can safely drive several properties or be
reused by another builder. The available Master features are `rms`, `peak`, and
linear-power `band(min_hz, max_hz)`. Features are raw linear measurements, not
automatically normalized to `0..1`, so a practical chain commonly uses gain,
remap, and clamp before it drives a visual property.

```python
bass = (
    builder.audio.master.band(40, 160)
    .gain(1.5)
    .remap(0.0, 0.2, 0.0, 1.0)
    .clamp(0.0, 1.0)
    .envelope(0.02, 0.18)
    .response_curve(0.42, 0.0, 0.58, 1.0)
)

clip.transform.scale.react_to(
    bass.remap(0.0, 1.0, 1.0, 1.08),
    mode="multiply",
)
glow = clip.effects.add_glow(threshold=0.25, radius=8.0, intensity=0.0, colour="#ffffff")
glow.intensity.modulate(bass.remap(0.0, 1.0, 0.0, 4.0), mode="replace")
```

`modulate` supports `replace`, `add`, and `multiply`, in declaration order.
Uniform scale reaction applies the same signal to `scale_x` and `scale_y`.
Target domains still apply after modulation: opacity stays in `0..1`, blur
radius stays in its supported range, and scale remains positive. That runtime
constraint is separate from an explicit signal `clamp`.

Bands use Hz. At 48 kHz, band energy is calculated from a 4096-sample Hann
STFT with a 10 ms hop and reports normalized linear power, not dB. Envelope
attack and release are one-pole time constants in seconds, precomputed during
preparation. Response curves clamp inputs at or below zero to zero and inputs
at or above one to one; their interior is a cubic Bezier response and may
overshoot when its y controls do. Put it after a normalized remap and clamp
when that is the desired shape.

Rotation properties and effect angles use degrees: transform rotation,
DirectionalBlur, ChromaticAberration, and CameraShake all retain those public
units. Signal analysis happens during `prepare`; prepared signal samples are
then reused for random-access frames and video operations. Changing an audio
file after preparation does not recompute visual reactions automatically.

`output_audio=False` disables muxed output audio only. Authored Master audio
remains available for visual signal analysis. A runnable deterministic reference
scene is available at `examples/python/08_audio_reactive.py`; it synthesizes its
own Master fixture and renders bass-to-scale, RMS-to-Glow, and high-band
Chromatic Aberration reactions without muxing audio.

Time domains are intentional: `clip.start`, transition starts, flash starts,
and post-effect keyframes are project-relative. Transform, opacity, crop, and
ordinary clip-effect keyframes are clip-local. Camera-shake tracks are relative
to their active interval. Preset timing follows the native preset's clip-local
active interval. Presets and timeline helpers are deterministic convenience
operations over the same canonical model as the low-level typed primitives.

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
  `InspectAssets`, `InspectAudio`, `InspectAudioTrack`, `InspectAudioClip`,
  `InspectAudioGainKeyframe`, `VersionResult`,
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

Phase 7C adds synchronous video rendering, progress callbacks, cancellation,
and immutable render results. FFmpeg is required for video rendering; FFprobe
is used by verification where applicable. Prepared frame rendering does not
encode a video. WGPU availability depends on the runtime adapter.

```python
import video_editor

editor = video_editor.Editor()
project = video_editor.Project.load("project.json")
prepared = editor.prepare(
    project,
    video_editor.PrepareOptions(backend=video_editor.BackendPreference.CPU),
)
result = prepared.render_video(
    video_editor.PreparedVideoRenderRequest("result.mp4", overwrite=True),
    progress=lambda event: print(event.kind, event.progress),
    cancellation=video_editor.CancellationToken(),
)
print(result.output_path)
```

Callbacks run synchronously on the calling render thread. Rendering methods
block that thread but release the interpreter between callbacks. The callback
receives only pre-publication `started` and `progress` events; a successful
return is the completion notification. A callback exception cancels the native
operation, waits for cleanup, and is then re-raised unchanged.

Phase 7A provides immutable project and inspection APIs. Phase 7B adds
preparation and single-frame rendering. Phase 7C completes the Python video
execution API.

```bash
python3 -m venv .venv
.venv/bin/python -m pip install maturin pytest mypy
.venv/bin/maturin develop
.venv/bin/python -m pytest python-tests
```

Use `import video_editor`. `Project.from_dict()` follows the same native path
as JSON. It is a lower-level way to construct the same current canonical
schema-version 2 model, useful for canonical JSON, low-level integrations, and
generated project dictionaries. It represents ordered multi-track audio timelines, renders arbitrary valid
static multi-input mixes, and accepts the schema-v2 scalar signal/modifier
surface used by audio-reactive visuals. Video assets and nested compositions
remain future work. Package path properties return `pathlib.Path` values. The
binding audits are recorded in
`docs/audits/phase7a.md`, `docs/audits/phase7b.md`, and
`docs/audits/phase7c.md`; together they record Phase 7 finalization.

The exposed Python API is immutable. `Project.from_dict()` accepts
`collections.abc.Mapping` values. Python does not expose asynchronous or
background rendering.
