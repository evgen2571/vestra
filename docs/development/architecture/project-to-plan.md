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

There are known validation inconsistencies: pure validation currently uses
visual/flash span for global-effect keyframe bounds and its initial timeline
budget check, before preflight resolves explicit/audio duration. These are
tracked in [global-effect timing](https://github.com/evgen2571/vestra/issues/8)
and [effective-output budgets](https://github.com/evgen2571/vestra/issues/9).
Do not describe their intended fixes as already implemented. Default limits
still include 216,000 frames and 7,200 seconds; the
[cap-removal ticket](https://github.com/evgen2571/vestra/issues/7) is future work.
Finite resource checks and representability remain separate concerns.

Minimum duration, automatic child-group inference, fill-to-end intent and
selected-range video output are not established APIs. For timing changes,
reuse the [timeline/media regression seams](../testing.md#timeline-and-media-checks)
and [contract-sync skill](../../../.agents/skills/api-contract-sync/SKILL.md).
