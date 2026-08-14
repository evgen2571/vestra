# Python editing API v2 parity and architecture audit

This audit records where each requested Python/native capability lives. It is
intended to prevent a feature from disappearing behind the new recommended
`vestra.Project` name.

| Feature | Recommended high-level API | Native or advanced path | Status and limits |
| --- | --- | --- | --- |
| Mutable project graph | `vestra.Project` | `vestra.authoring.ProjectBuilder` | High-level. Project owns root composition. |
| Immutable project | `project.snapshot()` and `vestra.ProjectSnapshot` | `vestra._native.Project` | Native snapshot. `ProjectSnapshot` is the stable public name. |
| Composition and layer ownership | `project.root`, `scene.add`, `scene.group` | Native Group clips | High-level. Child timing remains local and IDs are composition-scoped. Layers are not cloned or reparented; reused sources are copied into independent placements. |
| Image | `vestra.sources.Image` | Image asset and ImageClip | High-level. Supports sizing and crop. |
| Solid color | `vestra.sources.Color` or `SolidColor` | SolidColorClip | High-level. Full-canvas native source. |
| Spectrum2D | `vestra.sources.Spectrum2D` | Spectrum2DClip and native renderer | High-level. Uses the existing preset/override model. |
| ParticleSystem | `vestra.sources.ParticleSystem` | ParticleSystemClip and native renderer | High-level. Typed particle definition and optional audio reactivity. |
| Future Video, Text, Rectangle | Not implemented in this phase | Native representation and renderer still required | Extension guide documents the additive steps. |
| Timing and z-order | `Layer.start`, `duration`, `z` | Canonical clip timing | High-level. Layer-local placement values are copied into snapshots. |
| Transform, opacity, visibility, blend | Layer properties | Native tracks and evaluator | High-level. Capability adapters wrap sources that lack direct native presentation. |
| Typed keyframes and easing | `ScalarProperty`, `PointProperty`, `Transform`, `Interpolation`, `CubicBezier` | Authoring tracks and evaluator | High-level. Generic handles support future source and effect properties. |
| Effects | `Layer.effects`, `EffectStack` | Native effect catalog and renderer | High-level. Layer and project scopes are checked; clip-only effects remain clip-only. |
| Signals and audio reactivity | `project.audio.signal`, `ScalarSignal`, `.bind()` | Native master analysis and modifiers | High-level. `rms`, `peak`, and band energy are available; whole-scale and independent position/scale component bindings lower to native modifiers; analysis occurs during prepare. |
| Audio tracks and clips | `AudioTimeline`, `AudioTrack`, `AudioClip` | Canonical audio mixer and FFmpeg | High-level. Gain, mute, trims, fades, automation, crossfade, and supported EQ/bass/speed effects are preserved. |
| Transitions | `Composition.transitions.add()` | Canonical transition compiler and renderer | High-level with limits. Root composition only. Direct native endpoints are Image/Group; current Color/Particle/Spectrum paths use the capability adapter where valid. Nested transitions are rejected. |
| Cinematic presets | `Layer.presets`, `Preset` | ImageClip presets and native compiler | High-level. Image-only and at most one preset per layer. |
| Flashes | `Project.flashes`, `Flash` | Root visual overlays | High-level. Root-only. |
| Post-effects | `Project.post_effects` | Root post-effect collection | High-level. Root-only; native scope rules are retained. |
| Validation | `project.validate()` | Native `Editor.validate`, `ValidationReport` | High-level convenience over authoritative native diagnostics. |
| Snapshot | `project.snapshot()` | `ProjectSnapshot` | High-level lowering to immutable native data. A later editor mutation does not mutate an earlier snapshot. |
| Preparation | `project.prepare(backend=...)` | `PreparedProject`, `PreparationReport` | High-level convenience over native preparation. Prepared assets and audio remain tied to the prepared snapshot and readable input files. |
| Direct render | `project.render(path, backend=..., ...)` | Rust SDK `Editor.render` and `RenderResult` | High-level. No CLI subprocess. Result reports the selected backend and fallback. |
| Single-frame preview | `project.render_frame(seconds)` | `PreparedProject.render_frame_seconds/ns/number` | High-level convenience plus efficient prepared random access. |
| Video render from a prepared project | Native `PreparedProject.render_video` | `PreparedVideoRenderRequest` | Native runtime wrapper. Use for repeated operations after one preparation. |
| Progress | `project.render(progress=callback)` | Native render events | High-level callback receives pre-publication events. The binding filters native `completed`; successful result return is authoritative. |
| Cancellation | `CancellationToken` passed to `render` | Rust SDK cancellation | Public native workflow, surfaced by the high-level render call. |
| Structured diagnostics | `ValidationReport`, native exception classes and diagnostics | `vestra._native` DTOs | Public wrappers retain codes, categories, pointers, hints, and warnings. |
| Backend selection | `backend="auto"`, `"cpu"`, or `"wgpu"` | `BackendPreference`, `BackendKind`, adapter/fallback reports | High-level. CPU and WGPU reports are truthful. `wgpu` does not silently fall back; `auto` may fall back before first frame. |
| Frame bytes | `Frame.to_bytes()` | Native `Frame`, RGBA8 metadata | Public native DTO. NumPy and buffer protocol are intentionally not required. |
| Advanced exact canonical authoring | Not the normal editor path | `vestra.authoring.ProjectBuilder` | Advanced-only by design. Needed for exact schema handles, fixtures, lowering, and canonical schema work. |
| Low-level binding | Not the normal user path | `vestra._native` | Implementation-level. High-level wrappers keep PyO3 thin. |
| Removed legacy package | No supported import | `video_editor.Project` → `vestra.ProjectSnapshot` | The old package is no longer shipped. Native-project users must import `vestra.ProjectSnapshot`; do not substitute mutable `vestra.Project` without reviewing semantics. |

## Architecture checks

- `Project` owns one root `Composition`.
- `Composition` owns sibling `Layer` objects and transition relationships.
- `Layer` owns placement and presentation; `Source` owns source-specific data.
- Effects are stack values, properties are generic typed handles, and signals
  bind to compatible properties rather than requiring one helper per property.
- Lowering centralizes IDs, asset deduplication, source dispatch, capability
  adaptation, recursion, effects, signals, transitions, overlays, and audio.
- The Rust SDK owns validation, preparation, frame execution, video rendering,
  cancellation, backend selection, and result reporting.
- PyO3 exposes the native workflow and releases the GIL around long operations;
  Python does not become a per-frame evaluator.

The intentionally advanced-only pieces are exact canonical construction and
implementation-level native access. They are not missing high-level features:
they are lower-level control points that the editor API lowers through.
