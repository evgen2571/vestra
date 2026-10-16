# Spec: Correctness and quality

Status: draft; test seams approved, awaiting publication approval.

## Problem Statement

Existing focused fixtures do not by themselves establish long-run stability, all temporal parity combinations, or future colour/HDR contracts.

## Solution

Extend high-value temporal/media/alpha/failure regressions, add gated long-run stress workloads and evaluate future quality requirements through explicit research.

## User Stories

1. As a video author, I want to select correct VFR and B-frame media at fractional output rates, so that source timing survives varied media.
2. As a maintainer, I want to stress long and repeated operations, so that duration-dependent retention is visible.
3. As a video author, I want to receive consistent temporal effects across CPU and WGPU, so that backend choice does not unexpectedly change the sequence.
4. As a maintainer, I want to inspect frames around effect and transition boundaries, so that temporal regressions are not hidden by one static sample.
5. As a video author, I want to receive actionable errors for corrupted media, so that I can repair input without losing valid output.
6. As a maintainer, I want to verify cleanup when decode, write or publication fails, so that successful frame rendering is not mistaken for completed output.
7. As a video author, I want to preserve alpha at transformed edges and nested masks, so that transparent compositing avoids contamination.
8. As a maintainer, I want to define colour-management requirements explicitly, so that a new pipeline does not silently change current appearance.
9. As a maintainer, I want to assess higher precision and HDR against reference material, so that quality claims include memory and device constraints.
10. As a video author, I want to understand preview and final resampling tradeoffs, so that speed choices do not masquerade as encoder quality.
11. As a maintainer, I want to keep software and hardware evidence distinct, so that support claims reflect tests actually performed.

## Implementation Decisions

- Keep CPU/software-WGPU/hardware-WGPU evidence separate and name actual adapters.
- Validate rendered sequences and output bytes, not just shader parsing or dispatch coverage.
- Use generated deterministic media and existing fixture/error-injection infrastructure.
- Preserve encoded byte-space straight-alpha defaults; colour management, higher precision, HDR and quality policy are proposals.
- Set tolerances from each operation and reference rather than weakening checks globally.

## Testing Decisions

- Existing public render/media operations and frame/sequence parity fixtures, separately run software and hardware verification, plus ffprobe for media timing.
- Test observable behavior and highest existing public seams. Core/backend tests supplement arithmetic, aliasing and device-resource invariants; avoid tests that mirror private control flow.
- Verification belongs to each delivery slice. Hardware evidence is separate from software-WGPU evidence; report checks that could not run as unverified.

## Out of Scope

- Declaring full hardware or temporal parity without corresponding evidence.
- Silently switching to linear light, HDR or a higher-precision default.
- Duplicating the exact arithmetic matrix owned by T10.

## Further Notes

These are established objectives and constraints, not decisions selecting a research proposal. Research tickets require a reviewed recommendation before production adoption. See the proposed breakdown for the exact acceptance criteria, genuine blocking edges and deferred implementation scope.

Planned slices: Q01, Q02, Q03, Q04, Q05, Q06, Q07, Q08.
Baseline: develop revision 4c77b44431c355322c5fcaf0481bf02fa0e18356. No production changes are part of this planning task.
