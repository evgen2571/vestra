# Project to plan

Core turns a portable canonical project into a renderer-independent plan, then
evaluates that plan at each frame time. File access and media-dependent facts
enter through SDK preflight, so project semantics stay independent of the
machine that renders them.

## Validate, compile and evaluate

1. **Deserialize and validate.** `vestra-core::project` defines the schema-v1
   model. Semantic validation checks IDs, timelines, effects, transitions,
   signals and output constraints without reading assets.
2. **Resolve the environment.** SDK preflight resolves project-relative paths,
   probes media, resolves duration and checks the requested operation.
   `ValidatedProject::plan_compile_input` supplies these facts through
   `PlanCompileInput`; core never opens the supplied paths.
3. **Compile once.** Core lowers the hierarchy, tracks, effects, transitions
   and audio timeline into plan data with resolved asset references and frame
   count. Planning diagnostics retain canonical JSON pointers.
4. **Evaluate per frame.** Core uses checked frame time, active intervals,
   keyframes, signals and transition progress to produce evaluated sources and
   presentation channels. CPU and WGPU consume this work with the same meaning.

Nested groups retain their composition-local timelines. Evaluation recurses
into the active child composition rather than flattening its timing into root
placements. It also resolves inherited or explicit local canvas extents and
normalized positions. Renderers use those extents for precomposition, sizing,
masks and effects; a sized group can exceed the output before parent scaling.
See [group spatial semantics](../../reference/nested-compositions.md#sized-group-coordinates-and-bounds).

Transitions become generic outgoing/incoming presentation channels. Effects
retain ordered attachment scope through descriptor-backed compilation and
evaluation; the [effect pipeline](effect-pipeline.md) details that boundary.

## Time domains and frame boundaries

Identify the domain of each value before comparing it:

| Domain | Meaning |
| --- | --- |
| Project | Root visual placement, audio placement and global post-effect time |
| Composition-local | Child placement and sibling transition time inside a group |
| Clip-local | Time since a placement begins; tracks and effect activity follow their owner contract |
| Source-media | Video `source_start + local_time × playback_rate`; audio trims/effects follow their own duration contract |
| Frame | Output frame index and the rational rate mapping it to presentation time |

For example, a five-second group starts at project second `2`. Its child video starts at
composition second `1`, plays for `3` seconds and samples from source second
`10` at half speed:

<figure class="diagram" markdown="span">
![Project seconds 3, 4, 5 and the excluded child end 6 align with composition seconds 1, 2, 3, 4; clip-local seconds 0, 1, 2, 3; and source seconds 10, 10.5, 11, 11.5.](../../assets/diagrams/nested-time-domains.svg)
<figcaption>At project second 4, subtract the group start and child start: the video is one second old and samples source second 10.5.</figcaption>
</figure>

Child starts remain authored in their composition's clock. The parent active
interval clips child execution; authored clip end, output end and source-media
EOF remain separate boundaries. A sibling transition must fit both endpoints
in its owning composition and does not independently extend output. The
[authoring timeline guide](../../concepts/timeline-and-time.md) covers placement.

### Preserve each rounding rule

Core converts authored non-negative seconds to the nearest nanosecond, then
uses checked integer rational calculations. Activity is half-open: `[start, end)`.

| Operation | Rule |
| --- | --- |
| `frame_count` | Round up to include a final partial frame interval. |
| `frame_time_nanos` | Floor the rational frame time to nanoseconds for evaluation. |
| `frame_at_duration` | Floor a timestamp's rational frame position to select a frame. |
| `frame_start_duration` | Round up to the earliest nanosecond that maps back to the requested frame. |

For frame `1` at `30/1` fps, evaluation uses `33,333,333` ns; the public frame
timestamp is `33,333,334` ns so `frame_at_duration` returns frame `1`.
Accumulating floating-point frame steps, rounding rational FPS to an integer,
or clamping arithmetic overflow changes boundary behavior. Surface overflow
errors instead.

Runtime video ranges use the same rational ceiling helper as continuous image
sequences. The SDK resolves `[start, end)` to an absolute frame range against
prepared metadata. The staged loop evaluates/submits only those project frame
indices and writes sink indices from zero. The reusable plan stays unchanged.
The encoder's final audio mix retains project-time effects before trimming and
rebasing; nearest-sample rounding uses the 48 kHz mixer grid. See
[output range behavior](../../reference/render-outputs.md#ranges-and-frame-boundaries).

## Duration resolution and validation authority

The canonical model supports `automatic` and `explicit` duration:

| Mode | Output span |
| --- | --- |
| Automatic | Maximum root clip/flash end and resolved audio end. Muted or zero-gain audio still contributes, even when output audio is disabled. Empty content is rejected. |
| Explicit | The authored output duration. Later content is clipped with `VESTRA-DURATION-TRUNCATED`; shorter content leaves a background/silent tail. |

A group placement has an authored duration; children do not infer it. In
high-level Python, an omitted non-video duration inherits the **whole**
containing explicit span, even with a nonzero start. This value is resolved when
added, rather than maintained as a fill-to-end constraint. Video omission can
use intrinsic media duration. See [duration examples](../../concepts/timeline-and-time.md#duration-is-not-an-end-timestamp).

Pure validation checks authored data without I/O. Preflight probes media,
checks source availability and reapplies timeline limits against the final
resolved duration. CLI `ve validate` includes its validation preflight target;
SDK `Editor.validate` is pure. Compare the same canonical input at both seams.
Inspection and preparation reports record resolved facts.

Global-effect keyframes use the explicit output span or the automatic
visual/flash/audio span. If an audio end needs media probing, pure validation
defers that upper bound while still checking track structure and values.
Preflight rechecks global effects against the final duration.

Duration and frame budgets apply to the explicit output span, including tails,
rather than content clipped beyond it. In automatic mode, pure validation
budgets the visual/flash span and preflight rechecks it with media-resolved
audio. Authored timing remains validated even when clipped. Defaults impose no
arbitrary frame or duration budget; caller-selected finite budgets still apply.
Checked arithmetic and the prepared project's `u64` nanosecond duration bound
apply independently. See [validation limits](../../reference/project-format.md#validation-layers).

For timing changes, reuse the [timeline/media regression seams](../testing.md#timeline-and-media-checks)
and [contract-sync skill](https://github.com/evgen2571/vestra/blob/develop/.agents/skills/api-contract-sync/SKILL.md).

## Core source ownership

Paths are relative to `crates/vestra-core/src/`. Start with the owning facade,
then its implementation modules:

| Responsibility | Location |
| --- | --- |
| Authored hierarchy, sources and presentation | `project/model/` |
| Semantic rules and diagnostic order | `validation.rs`, `validation/` |
| Effect declarations and effect-local semantics | `effect_definition/catalog_entries.rs`, `effects/<effect>.rs` |
| Compilation and resource tables | `plan/compiler/` |
| Compiled plan and frame-time evaluation | `plan/model/`, `plan/evaluation/` |
| Audio, time, sizing, tracks and active intervals | `plan/audio.rs`, `plan/time.rs`, `plan/sizing.rs`, `plan/tracks.rs`, `plan/schedule/` |
| Seeded particle compilation and random-access evaluation | `plan/particles/` |

Tests beside the owning modules cover local behavior; `plan/tests.rs` exercises
compilation and evaluation together.
