# Spec: Media pipeline and encoding

Status: historical pre-publication spec snapshot. Published spec: [#59](https://github.com/evgen2571/vestra/issues/59).

GitHub Issues is authoritative. This original draft is retained for traceability,
not implementation tracking. See [publication status and approved changes](README.md)
for the final priorities and research-to-implementation review gate.

## Problem Statement

Long renders and new output workflows need reliable media behavior and clear capability boundaries, while current encoding controls and video operation ranges are limited.

## Solution

Validate long-run publication, timestamps and source scaling, and investigate encoding options, hardware paths, range rendering and resumability as separate decisions.

## User Stories

1. As a video author, I want to choose reviewed encoder settings independently where useful, so that encoding cost and output quality match the workflow.
2. As a SDK caller, I want to keep existing MP4 defaults, so that new output capabilities preserve current integrations.
3. As a maintainer, I want to evaluate actual hardware encoder support, so that availability and quality claims are device-specific.
4. As a maintainer, I want to compare hardware decode with timestamp-correct software decode, so that decode acceleration does not change selected frames.
5. As a maintainer, I want to measure GPU transfer and conversion overhead, so that a zero-copy proposal is judged end to end.
6. As a video author, I want to reuse decoder work across many source placements, so that large edits avoid unnecessary churn.
7. As a maintainer, I want to evaluate existing render/encode overlap under load, so that pipeline tuning accounts for ordered writes and memory.
8. As a video author, I want to render multi-hour and large-file output with stable AV sync, so that long projects finalize into usable media.
9. As a video author, I want to request a reviewed selected-range workflow, so that I can export a portion without altering project meaning.
10. As a video author, I want to understand segment and resume guarantees, so that interrupted work can eventually be recovered safely.
11. As a maintainer, I want to prioritize additional output formats against real requirements, so that codec breadth does not outrun tested capability.
12. As a video author, I want to retain a valid destination when rendering fails, so that failed work does not destroy existing output.

## Implementation Decisions

- Keep probing, decode, audio, encoding and publication in media ownership; renderers continue to consume evaluated work.
- Preserve current MP4/H264/yuv420p defaults and safe temporary-output publication until an approved output contract changes them.
- Build on existing PTS-based selection, decoder reuse/prefetch and bounded ordered encode overlap.
- Selected-range and segmented outputs require explicit source/project/output-time semantics before API implementation.
- Hardware paths require actual device/codec evidence and preserve explicit-request error reporting.

## Testing Decisions

- Public prepared/full render operations with generated media and ffprobe packet/frame/audio checks, plus existing decoder and fault-injected sink/publication tests.
- Test observable behavior and highest existing public seams. Core/backend tests supplement arithmetic, aliasing and device-resource invariants; avoid tests that mirror private control flow.
- Verification belongs to each delivery slice. Hardware evidence is separate from software-WGPU evidence; report checks that could not run as unverified.

## Out of Scope

- Claiming zero-copy, hardware decode/encode or additional codecs are already supported.
- A new resume format, range API or arbitrary FFmpeg argument passthrough before review.
- Committing generated multi-hour or large-file outputs.

## Further Notes

These are established objectives and constraints, not decisions selecting a research proposal. Research tickets require a reviewed recommendation before production adoption. See the proposed breakdown for the exact acceptance criteria, genuine blocking edges and deferred implementation scope.

Planned slices: M01, M02, M03, M04, M05, M06, M07, M08, M09, M10.
Baseline: develop revision 4c77b44431c355322c5fcaf0481bf02fa0e18356. No production changes are part of this planning task.
