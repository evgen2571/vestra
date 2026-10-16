# Spec: Timeline and long-form correctness

Status: draft; test seams approved, awaiting publication approval.

## Problem Statement

Users cannot reliably describe or validate long compositions when timeline authority differs between semantic validation, media preflight and global effects.

## Solution

Remove arbitrary default timeline caps, fix reproduced validation inconsistencies, protect rational frame boundaries and investigate unresolved duration behaviors before changing contracts.

## User Stories

1. As a video author, I want to render timelines beyond two hours and 216000 frames, so that project length follows content rather than an arbitrary ceiling.
2. As a SDK caller, I want to set finite execution budgets, so that long-form support retains resource protections.
3. As a video author, I want to use automatic duration across visual, flash and audio content, so that the output includes the intended timeline.
4. As a video author, I want to clip content to an explicit output duration, so that unused content does not invalidate the effective output span.
5. As a video author, I want to animate global effects across the resolved project span, so that short visual content does not reject valid late keyframes.
6. As a Python author, I want to understand omitted duration in nested groups, so that child-local content has predictable placement.
7. As a video author, I want to understand layers that fill to the project end, so that nonzero starts do not create accidental overruns.
8. As a maintainer, I want to evaluate a minimum-duration mode against existing modes, so that any added contract solves a demonstrated need.
9. As a video author, I want to receive consistent transition and source-end errors, so that valid boundary placements are not rejected unexpectedly.
10. As a SDK caller, I want to access exact long-range fractional-rate frame boundaries, so that frame selection does not accumulate drift.
11. As a video author, I want to locate invalid nested media from diagnostics, so that the reported pointer identifies the clip to repair.
12. As a maintainer, I want to retain explicit truncation warnings, so that users can distinguish intended clipping from missing content.

## Implementation Decisions

- Retain core as the authority for project timing and renderer-independent evaluation; SDK preflight supplies media facts.
- Remove arbitrary default frame/duration ceilings while retaining representability checks and caller-selected finite resource budgets.
- Keep automatic duration structurally dependent on visual/flash/audio placement, including authored muted audio, unless a reviewed investigation recommends a change.
- Preserve explicit clipping warnings, composition-local time and half-open frame lifetimes.
- Minimum duration, fill-to-end intent, nested inference and EOF policy are research scope, not approved new API decisions.

## Testing Decisions

- Public validation, preflight, inspection and prepared random-access rendering, supplemented by checked core arithmetic tests; use existing SDK fractional-frame, preflight duration and Python group/video tests.
- Test observable behavior and highest existing public seams. Core/backend tests supplement arithmetic, aliasing and device-resource invariants; avoid tests that mirror private control flow.
- Verification belongs to each delivery slice. Hardware evidence is separate from software-WGPU evidence; report checks that could not run as unverified.

## Out of Scope

- Unlimited-memory or overflow-prone rendering.
- Changing the model to a universal rational-time type without demonstrated need.
- Implementing new duration modes or EOF semantics before their decision ticket is reviewed.

## Further Notes

These are established objectives and constraints, not decisions selecting a research proposal. Research tickets require a reviewed recommendation before production adoption. See the proposed breakdown for the exact acceptance criteria, genuine blocking edges and deferred implementation scope.

Planned slices: T01, T02, T03, T04, T05, T06, T07, T08, T09, T10.
Baseline: develop revision 4c77b44431c355322c5fcaf0481bf02fa0e18356. No production changes are part of this planning task.
