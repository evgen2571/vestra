# Spec: Performance measurement and optimization

Status: draft; test seams approved, awaiting publication approval.

## Problem Statement

Users need reliable evidence about speed and memory before selecting optimizations or comparing CPU and WGPU.

## Solution

Extend the existing benchmark/report system with measured memory and actionable profiles, capture trustworthy baselines and bound each optimization hypothesis to a representative workload.

## User Stories

1. As a maintainer, I want to reuse versioned CPU and hardware-WGPU baselines, so that performance claims compare equivalent work.
2. As a maintainer, I want to retain raw repeated samples and environment identity, so that small differences can be judged against measurement noise.
3. As a SDK caller, I want to distinguish preparation, frame work and encoding cost, so that I can target the actual bottleneck.
4. As a maintainer, I want to measure renderer and encoder process memory, so that engine estimates do not hide whole-process costs.
5. As a maintainer, I want to profile evaluation and frame-plan construction, so that reuse work is guided by measured overhead.
6. As a video author, I want to benefit from safe static reuse in grouped scenes, so that unrelated groups do not unnecessarily prevent caching.
7. As a maintainer, I want to assess allocation reuse without borrowing returned frame pixels, so that performance changes preserve ownership.
8. As a maintainer, I want to compare backend and worker configurations, so that throughput gains include their memory costs.
9. As a maintainer, I want to assess additional fusion against an unfused reference, so that pass reductions preserve ordering and rounding.
10. As a maintainer, I want to test a bounded ROI or tile prototype, so that sparse workloads can be evaluated without a general renderer rewrite.
11. As a maintainer, I want to assess SIMD on one measured CPU hotspot, so that portable scalar behavior remains a reliable reference.
12. As a maintainer, I want to account for simultaneous transient and retained resources, so that individual cache limits do not obscure peak overlap.

## Implementation Decisions

- Reuse existing versioned suites, compatibility checks, raw samples and RenderResult stage/resource data.
- Separate real hardware WGPU from software-adapter correctness and keep prepared/frame-only scopes separate from encoded end-to-end work.
- Require paired measurements, correctness references and resource tradeoff reporting for every proposed optimization.
- Do not commit personal machine captures as portable performance targets.
- Cache topology, fusion, scheduling, SIMD, ROI and resource policy changes remain hypotheses until their tickets demonstrate a benefit.

## Testing Decisions

- Existing benchmark runner/record/compare contracts and public RenderResult reports; focused prepared/null-sink and backend frame tests supplement stage isolation.
- Test observable behavior and highest existing public seams. Core/backend tests supplement arithmetic, aliasing and device-resource invariants; avoid tests that mirror private control flow.
- Verification belongs to each delivery slice. Hardware evidence is separate from software-WGPU evidence; report checks that could not run as unverified.

## Out of Scope

- A promised universal speedup or fixed portable throughput target.
- Rebuilding existing benchmarking or cache systems.
- General render graph, scheduler or allocator rewrites without evidence.

## Further Notes

These are established objectives and constraints, not decisions selecting a research proposal. Research tickets require a reviewed recommendation before production adoption. See the proposed breakdown for the exact acceptance criteria, genuine blocking edges and deferred implementation scope.

Planned slices: P01, P02, P03, P04, P05, P06, P07, P08, P09, P10, P11.
Baseline: develop revision 4c77b44431c355322c5fcaf0481bf02fa0e18356. No production changes are part of this planning task.
