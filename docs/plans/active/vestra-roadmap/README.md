# Vestra development roadmap planning

Status: completed — published and read-back verified; retained as a historical snapshot.
Branch: `develop`. Source baseline: `4c77b44431c355322c5fcaf0481bf02fa0e18356`.
Original draft commit: `43d40fb`. Publication and verification: 2026-10-16.

## Objective

Publish a prioritized GitHub Issues backlog with six workstream specs, small delivery tickets,
native hierarchy and genuine blocking dependencies, preserving existing architecture and contracts.

## Authority and retention

The [published master roadmap](https://github.com/evgen2571/vestra/issues/63) and its linked GitHub Issues are the authoritative backlog.
This directory preserves the original audit, spec drafts and proposed ticket breakdown for traceability
and review. It is a historical snapshot, not a second tracker. Original priorities/review prompts
remain in the draft files; the approved publication changes are recorded below.
Do not synchronize future issue state, assignees or implementation progress back into this snapshot.

The user explicitly requested retention on `develop` after publication. Remove the completed planning
artifacts in a separate final cleanup commit before merging `develop` into `main`, following
[PLANS.md](../../../../PLANS.md). Do not remove them yet. Future implementation work should use its
own scoped execution plan where required.

## Baseline and references

The [audit](audit.md) records source inspection, architecture references and executed validation
reproductions. [tickets.md](tickets.md) preserves the original numbered 50-ticket breakdown;
[roadmap.md](roadmap.md) preserves the original master draft. These are planning records,
not evidence that any feature has been implemented.

## Published workstreams

| Spec | Original draft | Tickets | P0 | P1 | P2 | P3 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| [Timeline and long-form correctness](https://github.com/evgen2571/vestra/issues/57) | [Snapshot](spec-timeline.md) | 10 | 2 | 7 | 1 | 0 |
| [Performance measurement and optimization](https://github.com/evgen2571/vestra/issues/58) | [Snapshot](spec-performance.md) | 11 | 0 | 4 | 5 | 2 |
| [Media pipeline and encoding](https://github.com/evgen2571/vestra/issues/59) | [Snapshot](spec-media.md) | 10 | 0 | 4 | 3 | 3 |
| [Architecture and maintainability](https://github.com/evgen2571/vestra/issues/60) | [Snapshot](spec-architecture.md) | 6 | 0 | 3 | 3 | 0 |
| [Correctness and quality](https://github.com/evgen2571/vestra/issues/61) | [Snapshot](spec-quality.md) | 8 | 0 | 5 | 2 | 1 |
| [Python API, tooling and developer experience](https://github.com/evgen2571/vestra/issues/62) | [Snapshot](spec-developer-experience.md) | 5 | 0 | 3 | 2 | 0 |
| **Total** | | **50** | **2** | **26** | **16** | **6** |

Publication comprises 50 delivery tickets, six specs and the master roadmap: **57 issues**.
Eighteen tickets deliver implementation/verification; 32 deliver bounded research, prototypes or measurement.

## Scope and non-goals

Planning and publication only. No features, production code, public APIs or test fixtures changed.
No hardware benchmark, multi-hour encode or full native/Python verification is implied by publication.

## Constraints and approved changes

- Existing core/SDK/renderer/media ownership, local time, alpha, effect ordering, resource lifetimes
  and Rust/Python/JSON/CLI contracts remain constraints of future tickets.
- The user approved the existing test seams, six workstreams and all 50 proposed tickets.
- Only T02/[#8](https://github.com/evgen2571/vestra/issues/8) and T03/[#9](https://github.com/evgen2571/vestra/issues/9) retain P0: both are reproduced urgent validation defects.
- T01/T04/T05/T06/T08/T09/T10 moved from P0 to P1; T07 minimum-duration design moved to P2.
  The original draft counts (10 P0, 19 P1, 15 P2, six P3) are historical; published counts are above.
- Every accepted research/prototype result must lead to a **separate implementation ticket**,
  reviewed/approved before publication/readiness and before production implementation starts.
  Link the follow-up to its research issue and spec. Deferred/rejected results retain rationale
  and revisit conditions. Completing research does not authorize adopting its approach.
- Priorities are in issue titles/bodies. Existing `bug`/`enhancement` category and triage labels
  are used; no competing label system was created.
- Specs and the master are containers, not blocking gates. Native blocked-by relationships
  identify prerequisites; unrelated fixes do not wait on umbrella refactors.
- All published bodies and agent-brief comments begin with the configured AI triage disclaimer.
- The existing closed issue #4 was neither reopened nor duplicated.

## Milestones

- [x] Audit source, architecture, tests, tracker, duplicates and prior rejections.
- [x] Confirm test seams and draft six specs plus 50 ticket contracts.
- [x] Obtain publication approval and apply the priority/research-gate changes.
- [x] Publish 50 tickets, 50 durable agent briefs, six specs and the master roadmap.
- [x] Establish 56 native hierarchy relationships and 16 native blocking relationships.
- [x] Read back all issue bodies, labels, briefs, links and relationships.
- [x] Preserve original drafts with historical status and a published issue map.
- [ ] Remove this completed snapshot in a separate final commit before merging to `main`.
  This deferred cleanup does not leave publication incomplete.

## Acceptance and verification results

- Remote read-back verified all 57 open published issues: exact approved titles/bodies and
  category/state labels; all 50 delivery tickets have exactly one matching agent brief.
- All six spec child lists and the master spec list match the approved hierarchy: 56 native edges.
- Every delivery ticket was queried for native blockers, including roots: 16 edges match exactly,
  with no extra blockers. The approved graph is acyclic. All referenced issue links resolve to
  existing issues; issue #4 remains closed.
- At verification, 37 delivery tickets were unblocked, including 15 implementation/verification
  tickets. This is a publication snapshot; use GitHub for the live frontier.
- Snapshot preservation check passed: all ten planning files remain and the original audit,
  spec, ticket and master-draft content is unchanged below the historical status banners.
  Documentation validation checked 329 local file links in 100 Markdown files with none missing.
- Before publication, four focused core timeline tests passed. The full core run passed 327
  unit tests plus one integration test; one manual test was ignored. Four temporary assertions
  reproduced the documented timeline/global-effect validation cases against unchanged core code.
- Hardware rendering/benchmarks, multi-hour output and full Python/workspace suites remain
  **unverified**; their future tickets specify the required evidence.

## Recommended initial frontier at publication

Start with T02/[#8](https://github.com/evgen2571/vestra/issues/8) and T03/[#9](https://github.com/evgen2571/vestra/issues/9) (P0). T01/[#7](https://github.com/evgen2571/vestra/issues/7) (P1 cap removal) can proceed independently.
T09/[#15](https://github.com/evgen2571/vestra/issues/15) and T10/[#16](https://github.com/evgen2571/vestra/issues/16) cover diagnostic location and long-range boundary tests.
P01/[#17](https://github.com/evgen2571/vestra/issues/17), P02/[#18](https://github.com/evgen2571/vestra/issues/18) and P03/[#19](https://github.com/evgen2571/vestra/issues/19) establish measurement evidence in parallel.
Coordinate ownership when independent tickets edit shared code. No implementation has started.

## Published ticket map

This fixed map links original draft IDs to their initial published issues. GitHub owns future changes.

| Draft ID | Published issue | Initial priority | Delivery type |
| --- | --- | --- | --- |
| T01 | [#7](https://github.com/evgen2571/vestra/issues/7) | P1 | implementation |
| T02 | [#8](https://github.com/evgen2571/vestra/issues/8) | P0 | bug |
| T03 | [#9](https://github.com/evgen2571/vestra/issues/9) | P0 | bug |
| T04 | [#10](https://github.com/evgen2571/vestra/issues/10) | P1 | investigation |
| T05 | [#11](https://github.com/evgen2571/vestra/issues/11) | P1 | investigation |
| T06 | [#12](https://github.com/evgen2571/vestra/issues/12) | P1 | investigation |
| T07 | [#13](https://github.com/evgen2571/vestra/issues/13) | P2 | investigation |
| T08 | [#14](https://github.com/evgen2571/vestra/issues/14) | P1 | investigation |
| T09 | [#15](https://github.com/evgen2571/vestra/issues/15) | P1 | bug |
| T10 | [#16](https://github.com/evgen2571/vestra/issues/16) | P1 | verification |
| P01 | [#17](https://github.com/evgen2571/vestra/issues/17) | P1 | measurement |
| P02 | [#18](https://github.com/evgen2571/vestra/issues/18) | P1 | implementation |
| P03 | [#19](https://github.com/evgen2571/vestra/issues/19) | P1 | implementation |
| P04 | [#20](https://github.com/evgen2571/vestra/issues/20) | P1 | prototype |
| P05 | [#21](https://github.com/evgen2571/vestra/issues/21) | P2 | prototype |
| P06 | [#22](https://github.com/evgen2571/vestra/issues/22) | P2 | prototype |
| P07 | [#23](https://github.com/evgen2571/vestra/issues/23) | P2 | investigation |
| P08 | [#24](https://github.com/evgen2571/vestra/issues/24) | P2 | prototype |
| P09 | [#25](https://github.com/evgen2571/vestra/issues/25) | P3 | prototype |
| P10 | [#26](https://github.com/evgen2571/vestra/issues/26) | P3 | prototype |
| P11 | [#27](https://github.com/evgen2571/vestra/issues/27) | P2 | investigation |
| M01 | [#28](https://github.com/evgen2571/vestra/issues/28) | P1 | investigation |
| M02 | [#29](https://github.com/evgen2571/vestra/issues/29) | P2 | prototype |
| M03 | [#30](https://github.com/evgen2571/vestra/issues/30) | P2 | prototype |
| M04 | [#31](https://github.com/evgen2571/vestra/issues/31) | P3 | investigation |
| M05 | [#32](https://github.com/evgen2571/vestra/issues/32) | P1 | investigation |
| M06 | [#33](https://github.com/evgen2571/vestra/issues/33) | P2 | prototype |
| M07 | [#34](https://github.com/evgen2571/vestra/issues/34) | P1 | verification |
| M08 | [#35](https://github.com/evgen2571/vestra/issues/35) | P1 | investigation |
| M09 | [#36](https://github.com/evgen2571/vestra/issues/36) | P3 | investigation |
| M10 | [#37](https://github.com/evgen2571/vestra/issues/37) | P3 | investigation |
| A01 | [#38](https://github.com/evgen2571/vestra/issues/38) | P1 | verification |
| A02 | [#39](https://github.com/evgen2571/vestra/issues/39) | P2 | investigation |
| A03 | [#40](https://github.com/evgen2571/vestra/issues/40) | P1 | investigation |
| A04 | [#41](https://github.com/evgen2571/vestra/issues/41) | P2 | investigation |
| A05 | [#42](https://github.com/evgen2571/vestra/issues/42) | P1 | verification |
| A06 | [#43](https://github.com/evgen2571/vestra/issues/43) | P2 | investigation |
| Q01 | [#44](https://github.com/evgen2571/vestra/issues/44) | P1 | verification |
| Q02 | [#45](https://github.com/evgen2571/vestra/issues/45) | P1 | verification |
| Q03 | [#46](https://github.com/evgen2571/vestra/issues/46) | P1 | verification |
| Q04 | [#47](https://github.com/evgen2571/vestra/issues/47) | P1 | verification |
| Q05 | [#48](https://github.com/evgen2571/vestra/issues/48) | P1 | verification |
| Q06 | [#49](https://github.com/evgen2571/vestra/issues/49) | P2 | investigation |
| Q07 | [#50](https://github.com/evgen2571/vestra/issues/50) | P3 | prototype |
| Q08 | [#51](https://github.com/evgen2571/vestra/issues/51) | P2 | prototype |
| D01 | [#52](https://github.com/evgen2571/vestra/issues/52) | P1 | implementation |
| D02 | [#53](https://github.com/evgen2571/vestra/issues/53) | P1 | investigation |
| D03 | [#54](https://github.com/evgen2571/vestra/issues/54) | P2 | prototype |
| D04 | [#55](https://github.com/evgen2571/vestra/issues/55) | P1 | implementation |
| D05 | [#56](https://github.com/evgen2571/vestra/issues/56) | P2 | implementation |

## Decisions and discoveries

2026-10-16: The audit found existing benchmarking/comparison, checked rational timing, CPU/GPU
resource reuse, decoder reuse/prefetch, limited effect fusion and bounded render/encode overlap.
The published backlog targets improvements or measured hypotheses instead of recreating them.

2026-10-16: Priority was narrowed to reproduced urgent defects, and accepted research was explicitly
gated on separately reviewed implementation tickets. All 57 issues and their relationships were
published and read back successfully. The user subsequently requested retention of the original
planning artifacts until the final pre-merge cleanup commit.

## Completion / handoff

Publication is complete. Continue from the [GitHub roadmap](https://github.com/evgen2571/vestra/issues/63) and the owning specs/agent briefs.
Open choices remain around nested/fill/minimum duration, source-end/range behavior, encoder/hardware
paths, evidence-dependent optimization/refactors, colour/HDR/resampling and public budget/editing APIs.
Their research tickets retain traceability and require reviewed implementation follow-ups.
Do not track future implementation here or remove this directory until the separate pre-merge
cleanup requested by the user. No feature implementation is authorized by this completed planning task.
