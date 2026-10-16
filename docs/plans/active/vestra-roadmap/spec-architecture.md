# Spec: Architecture and maintainability

Status: draft; test seams approved, awaiting publication approval.

## Problem Statement

Cross-surface integration and distributed resource policies can drift, while broad restructuring would threaten working behavior without a demonstrated benefit.

## Solution

Add targeted consistency/lifecycle guards and identify only justified reductions in duplicated decisions, ownership ambiguity or module coupling.

## User Stories

1. As a maintainer, I want to detect capability drift across public surfaces, so that support claims agree with canonical behavior.
2. As a effect author, I want to reuse existing descriptors and pass planning, so that integration does not duplicate semantic decisions.
3. As a maintainer, I want to identify redundant effect declarations with a concrete example, so that simplification has a demonstrated purpose.
4. As a SDK caller, I want to understand which owner enforces each resource policy, so that validation and execution apply consistent budgets.
5. As a maintainer, I want to split a large module only around a verified responsibility, so that refactoring reduces actual maintenance risk.
6. As a SDK caller, I want to retain rendered frames while preparing later work, so that returned pixel ownership is stable.
7. As a Python caller, I want to receive consistent busy and invalidated-state errors, so that misuse fails clearly instead of corrupting prepared state.
8. As a maintainer, I want to verify teardown after failure and cancellation, so that threads, processes and GPU resources remain safely owned.
9. As a effect author, I want to understand the fixed temporary-resource limits, so that a new effect can be assessed before expanding the IR.
10. As a maintainer, I want to require a demonstrated workload before an IR extension, so that the working architecture stays focused.

## Implementation Decisions

- Preserve existing crate dependency direction and the shared core semantics for CPU/WGPU.
- Use canonical descriptors and existing source capabilities as prior art instead of introducing a competing registry.
- Refactor only against a concrete integration, resource-policy or testability problem.
- Use independently green changes, and expand/migrate/contract only when a mechanical public migration actually needs it.
- Treat Rust/Python/JSON/CLI behavior, errors and support claims as compatibility contracts.

## Testing Decisions

- Existing public export, descriptor, schema, authoring and prepared-project lifecycle tests; backend-specific resource checks supplement observable ownership behavior.
- Test observable behavior and highest existing public seams. Core/backend tests supplement arithmetic, aliasing and device-resource invariants; avoid tests that mirror private control flow.
- Verification belongs to each delivery slice. Hardware evidence is separate from software-WGPU evidence; report checks that could not run as unverified.

## Out of Scope

- A blanket compiler/renderer rewrite or new plugin framework.
- Moving file/media/GPU operations into core.
- Changing the pass IR for hypothetical future features.

## Further Notes

These are established objectives and constraints, not decisions selecting a research proposal. Research tickets require a reviewed recommendation before production adoption. See the proposed breakdown for the exact acceptance criteria, genuine blocking edges and deferred implementation scope.

Planned slices: A01, A02, A03, A04, A05, A06.
Baseline: develop revision 4c77b44431c355322c5fcaf0481bf02fa0e18356. No production changes are part of this planning task.
