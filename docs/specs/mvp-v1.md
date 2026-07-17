# Declarative Video Renderer MVP, version 1

## Scope

`video-editor` is a standalone command-line declarative video renderer. It parses a versioned JSON project, validates local image and audio assets, evaluates a deterministic image timeline, and creates an MP4 video. It has no GUI, network assets, source-video clips, text, remote services, or multiple audio tracks.

## Normative contract

The supported project version is exactly `1`. Normative rendering objects reject unknown fields; the optional `metadata` object is ignored. JSON uses snake_case. Times are finite non-negative decimal seconds. Internally they are converted to nanoseconds using decimal text, then frames are sampled as `index / frame_rate`; no frame duration is repeatedly accumulated. A positive duration `d` contains `ceil(d × frame_rate)` frames. An item is active iff `start <= frame_time < start + duration`.

`frame_rate` is either a positive JSON number or a rational string such as `"30000/1001"`. Output dimensions are positive even values in the inclusive range 2..8192. Output is MP4/H.264 (`yuv420p`) and AAC when audio is enabled. Quality profiles are `preview`, `balanced`, and `high`; they map to fixed CRFs 30, 23, and 18.

All relative asset and project output paths resolve relative to the project file. The CLI `--output` path resolves relative to the process current directory. URLs and directories are invalid. Existing output requires `--overwrite`. Rendering writes a uniquely named temporary MP4 beside the destination and atomically publishes it only after FFmpeg succeeds.

## Timeline semantics

Every frame starts as the opaque `output.background` colour. Active visible clips sort by `(layer, start, id)` and are painted in ascending order. The same rule orders flashes, which are full-canvas source-over overlays. A clip transformation is crop, base sizing, animated scale, anchor placement, then opacity and transition opacity. `position` is normalized canvas coordinates (values outside zero through one are allowed); `anchor` is normalized cropped-image coordinates in zero through one.

Sizing modes are `original`, `fit`, `cover`, `scale`, and `stretch`. `original` uses cropped source pixels; `fit` and `cover` preserve aspect ratio against the canvas; `scale` preserves aspect ratio with a required positive scalar; and `stretch` requires positive width and height and is the only intentional distortion. Crop is normalized `{x,y,width,height}`, strictly within source bounds after conversion. Source edge coordinates use `floor(x * source_size)` and end coordinates use `ceil((x + width) * source_size)`.

Opacity is zero through one and uses source-over alpha composition. Animations target `position`, `scale`, `opacity`, or `crop`, have an explicit start/end value, clip-relative interval, and `linear`, `ease_in`, `ease_out`, or `ease_in_out` easing. The equations are `t`, `t²`, `1-(1-t)²`, and `3t²-2t³`. Same-target intervals may not overlap; later segments retain the previous end value until their own start. Transition contribution multiplies clip opacity. Crossfade identifies incoming and outgoing clips and uses the same easing; fade-to/from-background affects the named clip. A clip cannot have overlapping transitions.

Flash attack ramps from zero to peak, hold remains peak, and release ramps to zero. Attack plus release may not exceed duration.

Duration is explicit (`duration_mode: "explicit"` and `duration`) or automatic. Automatic duration is the maximum end of visual clips, flashes, and the enabled selected audio. Explicit duration clips content and leaves background/silence. Empty automatic timelines are invalid.

The optional primary audio track selects one audio asset with source trim, timeline placement, linear gain, fades, and mute. It is source-trimmed first; source start does not move visuals. Fades operate over the selected source interval. Silence is inserted before `timeline_start`; audio is limited to the project duration.

## CLI, structured results, errors

Commands are `validate`, `inspect`, `render`, `version`, and `help`. `--format json` emits one versioned JSON result to stdout; diagnostics use stderr. `render --progress json` emits JSON Lines events (`started`, monotonic `progress`, then `completed` or `failed`) to stdout. `--progress none` is silent. `--preview` scales dimensions down to at most 640 pixels on the longest side without changing timing and uses preview quality.

Exit codes: 0 success, 1 internal failure, 2 CLI usage, 3 malformed/unsupported/semantic project, 4 asset/media failure, 5 backend/render failure, 6 output failure, 130 cancellation.

Errors have stable code, category, message, JSON Pointer where known, and optional hint. Validation reports independent errors where possible. Warnings include unused assets, hidden/zero-opacity clips, tiny content, explicit-duration truncation, no-op animation, and animation discontinuity.

## Backend decision

Rust owns project parsing, validation, path resolution, timeline evaluation, image composition, progress, process orchestration, and output safety. FFmpeg 7+ receives incrementally streamed RGBA frames and performs H.264/AAC encoding and muxing. FFprobe reads audio duration. This avoids codec implementation while retaining deterministic visible-frame control.

## Requirements traceability

| Requirement | Implementation | Evidence | Status |
|---|---|---|---|
| MVP-PROJECT-001: versioned JSON, schema, strict fields | `project`, `schemas/project-v1.schema.json` | parser/validation tests | planned |
| MVP-TIME-001: deterministic frame timing and duration | `timeline` | unit tests | planned |
| MVP-VISUAL-001: image compositing, crop, transforms, animation | `render` | pixel and end-to-end tests | planned |
| MVP-AUDIO-001: one trimmed, faded primary track | `media`, `render` | FFprobe end-to-end tests | planned |
| MVP-CLI-001: validate, inspect, render and JSON events | `main`, `cli` | integration tests | planned |
| MVP-SAFETY-001: temporary output and overwrite protection | `render` | integration tests | planned |
| MVP-DOCS-001: runnable examples and docs | `examples`, `README` | example validation test | planned |
