# Project to plan

The canonical project is portable JSON, not a render-ready scene. `vestra-core::project` deserializes its schema-v1 model and semantic validation checks IDs, timelines, output constraints, effect scopes, transition endpoints, signals and other relationships without reading assets. The SDK preflight layer then resolves relative paths against the project base directory, probes media, computes media-dependent duration facts, checks the operation target, and passes those facts to compilation.

```text
canonical Project
  -> semantic ValidationReport
  -> SDK preflight and resolved media facts
  -> PlanCompileInput / compiled plan
  -> evaluated frame plan at time t
  -> backend frame work
```

Compilation exists so cross-clip work happens once. It turns the validated visual hierarchy, audio timeline, descriptor-backed effects, transitions, tracks and warnings into renderer-independent plan data. It normalizes timeline authority, frame count and asset references before a renderer can see them. `ValidatedProject::plan_compile_input` is the SDK boundary that carries resolved paths, audio/video duration and dimensions into the core compiler; core itself does not touch files or media tools.

Evaluation remains time-dependent. For each frame it uses the checked frame time, active intervals, keyframes, signal values and transition progress to produce evaluated layers and source work. Renderers receive those evaluated sources and presentation channels, not public source objects. This matters for nested compositions: a group has its own local timeline, compilation preserves that ownership, and evaluation recurses into it instead of flattening away its timing rules.

Transitions lower to generic outgoing/incoming presentation channels. Effects lower through the descriptor/catalog representation and retain ordered attachment scope. Both mechanisms stay in core planning/evaluation so CPU and WGPU apply the same meaning. Planning diagnostics retain canonical JSON pointers; preflight diagnostics add the environment or media fact that failed.

## Core source ownership

Paths below are relative to `crates/vestra-core/src/`. Public types remain
available through the existing `project`, `plan` and descriptor exports;
private child modules organize their implementations.

| Responsibility | Location |
| --- | --- |
| Authored hierarchy and presentation | `project/model/visual.rs` |
| Authored particle and spectrum sources | `project/model/particles.rs`, `project/model/spectrum2d.rs` |
| Ordered semantic validation | `validation.rs`, `validation/visual.rs`, `validation/effects.rs` |
| Visual source, transform and mask/matte rules | `validation/visual/` |
| Effect attachment, parameter and property rules | `validation/effects/` |
| Compilation coordination and source/resource tables | `plan/compiler/mod.rs`, `plan/compiler/assets.rs` |
| Matte identities and temporal dependencies | `plan/compiler/mattes.rs` |
| Compiled plan/layers and compiled effects | `plan/model/mod.rs`, `plan/model/effects.rs` |
| Particle models, compilation and random-access evaluation | `plan/particles/` |
| Audio planning, time conversions, sizing and track lowering | `plan/audio.rs`, `plan/time.rs`, `plan/sizing.rs`, `plan/tracks.rs` |
| Active-layer schedules and raw interval events | `plan/schedule/` |

The root `plan_audio`, `plan_time`, `plan_sizing`, `plan_tracks` and
`plan_schedule` paths are compatibility aliases. Raw interval events retain
their index-based representation; active-layer schedules use `ScheduledItem`.
Validation's hierarchy walk controls diagnostic order, including nested error
rebasing and the composition-wide matte checks after all clips. Extracted rule
modules append to the same diagnostic list in that order.

Particle compilation builds immutable models and lifetime curves. Evaluation
derives identities, motion and appearance from local time and explicit seeds;
it keeps no frame-order state. Tests live beside the owning implementation,
while `plan/tests.rs` exercises compilation and evaluation together.

## Time domains and frame boundaries

The [authoring timeline guide](../../concepts/timeline-and-time.md) describes
placement. Identify the domain of each value before comparing it:

| Domain | Meaning |
| --- | --- |
| Project | Root visual placement, audio placement and global post-effect time |
| Composition-local | Child placement and sibling transition time inside a group |
| Clip-local | Time since a placement begins; local tracks/effect activity follow their owner contract |
| Source-media | Video `source_start + local_time * playback_rate`; audio trims/effects follow their own duration contract |
| Frame | Output frame index and the rational rate mapping it to presentation time |

Nested child starts remain local; parent placement does not rewrite their
authored values. Execution clips children to the containing active interval.
Authored clip end, effective output end and source-media EOF are different
boundaries. A transition must fit both endpoint lifetimes in its owning
composition; it does not independently extend the output.

Core timeline helpers use checked integer rational calculations after converting
authored seconds to nanoseconds. Activity is half-open, `[start, end)`, and frame
count rounds up. `frame_time_nanos` floors evaluation time to nanoseconds;
`frame_start_duration` rounds the public frame timestamp up so mapping it with
`frame_at_duration` selects that frame. Preserve these operation-specific
rounding rules. Accumulating floating-point frame steps, rounding FPS to 30,
or replacing an overflow error with clamping changes boundary behavior.

## Duration resolution and validation authority

The canonical model currently supports `automatic` and `explicit` duration:

- Automatic preflight uses the maximum root clip/flash end and resolved audio
  end. Authored muted/zero-gain audio still contributes structurally, even when
  output audio is disabled. Empty automatic content has no positive duration.
- Explicit duration fixes the output span. Later content is clipped with
  `VESTRA-DURATION-TRUNCATED`; shorter content can leave a background/silent tail.
- A group placement has an authored duration; child content does not currently
  infer it. In high-level Python, omitted non-video durations inherit the whole
  containing explicit span, including when start is nonzero. This is not a
  persistent fill-to-end constraint. Video omission can use intrinsic media.

Pure validation checks canonical data without I/O. SDK preflight probes media,
resolves duration, checks source availability and reapplies final timeline
limits. CLI `ve validate` includes its validation preflight target, whereas
SDK `Editor.validate` is pure; compare the same canonical input at both seams.
Inspection/preparation reports contain resolved facts, not merely authoring intent.

Global-effect keyframes use the explicit output span, or the automatic
visual/flash/audio span. Pure validation checks authored timing without I/O;
when an audio end depends on media probing, it defers the upper bound while
still checking track structure and values. Preflight rechecks global effects
against the final resolved project duration before preparation.

Timeline duration and frame budgets apply to the explicit output span, including
any background/silent tail, rather than content clipped beyond that span. In
automatic mode, pure validation budgets the visual/flash span; preflight
rechecks the final span including media-resolved audio. Authored timing remains
validated even when content is clipped. Defaults impose no arbitrary frame or
duration budget; caller-selected finite budgets still apply. Checked arithmetic
and the prepared project's `u64` nanosecond duration bound remain enforced
independently of those budgets. See [validation limits](../../reference/project-format.md#validation-layers).

Minimum duration, automatic child-group inference, fill-to-end intent and
selected-range video output are not established APIs. For timing changes,
reuse the [timeline/media regression seams](../testing.md#timeline-and-media-checks)
and [contract-sync skill](../../../.agents/skills/api-contract-sync/SKILL.md).
