# Spec: Python API, tooling and developer experience

Status: historical pre-publication spec snapshot. Published spec: [#62](https://github.com/evgen2571/vestra/issues/62).

GitHub Issues is authoritative. This original draft is retained for traceability,
not implementation tracking. See [publication status and approved changes](README.md)
for the final priorities and research-to-implementation review gate.

## Problem Statement

Users need to see resolved timelines, actual backend capabilities and actionable performance data without reconstructing internal execution rules.

## Solution

Extend shared inspection/discovery/report surfaces and investigate minimal editing/resource configuration APIs while keeping authoring intent distinct from resolved execution.

## User Stories

1. As a Python author, I want to inspect resolved output duration and frame count, so that authoring intent is distinguishable from effective execution.
2. As a video author, I want to inspect nested local and project intervals, so that I can understand placement and clipping without rendering every frame.
3. As a SDK caller, I want to inspect source-time mapping and associated diagnostics, so that timing errors are actionable.
4. As a SDK caller, I want to set reviewed resource budgets consistently, so that validation and preparation do not silently use different limits.
5. As a Python author, I want to evaluate one concrete timeline editing helper, so that convenience does not create an oversized editor framework.
6. As a SDK caller, I want to discover available backends and capabilities, so that I can choose a feasible render path before expensive work.
7. As a video author, I want to see backend selection and fallback reasons, so that the reported backend reflects actual execution.
8. As a SDK caller, I want to distinguish unsupported, unavailable and unverified operations, so that discovery does not overstate platform support.
9. As a video author, I want to read coherent timing and memory reports, so that I can diagnose the cost of a render.
10. As a Python caller, I want to use report fields whose types match native results, so that tooling remains dependable.
11. As a maintainer, I want to ship relevant examples with behavioral changes, so that documentation demonstrates the feature it explains.

## Implementation Decisions

- Keep Python a thin authoring/binding adaptation over the shared SDK; do not add a second timeline authority.
- Extend existing inspection and RenderResult contracts compatibly.
- Distinguish unknown facts, unsupported capabilities and unverified hardware behavior.
- Resource configuration and editing helpers require reviewed contracts rather than an invented API family.
- Examples, typing, exports and diagnostics ship with their behavior-changing ticket; avoid detached documentation-only chores.

## Testing Decisions

- SDK/CLI/Python inspection, discovery and report contract tests, including typing/export checks and high-level lowering fixtures; reuse native frame/media tests for execution.
- Test observable behavior and highest existing public seams. Core/backend tests supplement arithmetic, aliasing and device-resource invariants; avoid tests that mirror private control flow.
- Verification belongs to each delivery slice. Hardware evidence is separate from software-WGPU evidence; report checks that could not run as unverified.

## Out of Scope

- Building a GUI editor or a separate Python renderer.
- A broad convenience API without a concrete editing workflow.
- Duplicating duration-inference work owned by the timeline spec.

## Further Notes

These are established objectives and constraints, not decisions selecting a research proposal. Research tickets require a reviewed recommendation before production adoption. See the proposed breakdown for the exact acceptance criteria, genuine blocking edges and deferred implementation scope.

Planned slices: D01, D02, D03, D04, D05.
Baseline: develop revision 4c77b44431c355322c5fcaf0481bf02fa0e18356. No production changes are part of this planning task.
