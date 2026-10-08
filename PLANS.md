# Implementation plans

Use an execution plan for large features, resource/architecture changes,
cross-component refactors or work spanning multiple sessions. Small fixes
need no special plan. This convention makes work resumable without chat history.

Plans live under `docs/plans/active/<topic>.md` until complete. Move finished
plans to `docs/plans/completed/<topic>.md` and update links. Do not keep
conflicting active plans for the same work.

Current active plan: [stylized video effects](docs/plans/active/stylized-video-effects.md).

## Required sections

1. **Objective:** user-visible result and motivation.
2. **Baseline:** branch/revision, verified existing behavior, and relevant
   source-of-truth code/document links; distinguish facts from guesses.
3. **Scope and non-goals:** what changes and what is out of scope.
4. **Constraints and decisions:** API, semantics, architecture, resource
   costs, failure behavior, and unresolved choices.
5. **Milestones:** independent deliverables and the acceptance evidence for
   each, not merely a list of files to edit.
6. **Acceptance criteria:** validation, edge cases, backend behavior,
   examples, performance, documentation and compatibility.
7. **Progress and verification:** completed work, actual commands/results,
   blockers and unverified checks.
8. **Decisions/discoveries:** short dated changes in assumptions or tradeoffs.
9. **Handoff:** final outcome, known limitations and unfinished tasks.

## Updating a plan

Read `AGENTS.md`, current implementation and relevant documentation first.
Finalize contracts before large incompatible implementations. Update a plan
at meaningful milestones or decisions, not on every edit. Mark a milestone
complete only when acceptance is demonstrated. Distinguish **done** from
**blocked** and **not verified**, especially for hardware-only tests.

Link canonical documentation rather than copying it. Put feature-specific
decisions in the plan. Avoid audit/report bureaucracy that does not help
future implementation.

## Plan template

```markdown
# Feature title
Status: planned | in progress | blocked | completed
Branch / baseline:

## Objective
## Baseline and references
## Scope and non-goals
## Constraints and decisions
## Milestones
- [ ] Milestone A — deliverable and validation
- [ ] Milestone B — deliverable and validation
## Acceptance criteria
## Progress and verification
## Decisions and discoveries
## Completion / handoff
```
