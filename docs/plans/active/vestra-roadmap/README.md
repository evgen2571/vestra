# Vestra development roadmap planning

Status: in progress — audit and drafts complete; awaiting publication approval.
Branch / baseline: `develop`, `4c77b44431c355322c5fcaf0481bf02fa0e18356`.

## Objective

Publish a complete prioritized GitHub Issues backlog for the next development
stages, with a master roadmap, six workstream specs, bounded delivery tickets,
native hierarchy/dependencies and an immediately available frontier.

## Baseline and references

Read [the audit](audit.md), [the proposed ticket contracts](tickets.md) and
the six specs:

- [Timeline and long-form correctness](spec-timeline.md)
- [Performance measurement and optimization](spec-performance.md)
- [Media pipeline and encoding](spec-media.md)
- [Architecture and maintainability](spec-architecture.md)
- [Correctness and quality](spec-quality.md)
- [Python API, tooling and developer experience](spec-developer-experience.md)

The GitHub tracker contains no open issues and one closed issue, #4. Its full
body and comments were read. Native sub-issue and blocking-dependency read
endpoints both succeed. Write capability is not established by a successful read.

## Scope and non-goals

Planning only. Production code, public APIs and source fixtures remain unchanged.
Drafts distinguish reproduced bugs, source-confirmed limitations, existing
functionality and proposals. No long benchmark, hardware run or feature
implementation is implied by this audit.

## Constraints and decisions

- Follow the installed `to-spec`, `to-tickets` and `triage` skills.
- The user approved the proposed existing test seams on 2026-10-16.
- Wait for review of the numbered ticket breakdown before publication.
- Preserve crate ownership, local time, alpha, effect ordering, resource
  lifetimes, diagnostics and Rust/Python/JSON/CLI compatibility.
- Use configured `bug`/`enhancement` and triage labels. Record priority in
  titles/bodies rather than creating another label vocabulary.
- Research tickets can be ready for a bounded investigation; that label does
  not authorize adoption of their proposed APIs or algorithms.
- Specs state agreed objectives and constraints. Investigation results require
  a later implementation decision; no alternative is silently selected.
- Do not modify or close existing parent issues. Publish newly created specs
  with final child links, then publish the master with final spec links.
  Add native relationships separately and retain explicit dependency/parent
  references when native writes are unavailable.

## Milestones

- [x] Audit source, architecture, tests, tracker and duplication/prior rejection.
- [x] Confirm test seams with the user.
- [x] Draft six specs and 50 prioritized tickets with acceptance/verification.
- [ ] Obtain breakdown/granularity/blocker approval before publication.
- [ ] Recheck tracker/source identity for intervening changes and duplicates.
- [ ] Publish approved tickets in dependency order, then linked specs and master.
- [ ] Post required durable agent briefs, attach native hierarchy/dependencies,
      and verify labels, bodies, counts, all links and the startable frontier.
- [ ] Report public issue URLs, counts, recommended first tickets and open decisions.

## Acceptance criteria

Every scoped improvement has one primary owning ticket or explicit research
traceability. Each ticket defines outcome, evidence, acceptance, verification,
priority, category/state and genuine blockers. Unrelated work has no artificial
umbrella dependency. No speculative speed guarantee or unsupported hardware
claim is published. Existing issues remain unchanged. Publication is complete
only after remote bodies, labels, hierarchy and blocking edges are read back.

## Progress and verification

- `git status --short --branch`: clean `develop` at start.
- CodeGraph reports this checkout is not indexed; source inspection used `rg`
  and direct reads instead.
- `gh issue list --state all --limit 1000 ...`: one closed issue, no comments.
- GET #4 sub-issues and blocked-by endpoints: both returned empty lists.
- `cargo test --offline -p vestra-core timeline::tests -- --nocapture`:
  four tests passed.
- `cargo test --offline -p vestra-core`: 327 unit tests plus one integration
  test passed; one manual test ignored.
- A temporary assertion-based Rust reproducer called the unchanged core
  validation API and confirmed four timeline/global-effect validation cases;
  details are in the audit. It is not a committed production test or fix.
- Documentation-link check: 319 local file links across 100 Markdown files,
  zero missing. `git diff --check` passes.
- Draft self-check: six spec templates, at least ten user stories per spec,
  50 unique complete ticket contracts and 16 acyclic blocking edges.
- Hardware rendering, benchmarks, multi-hour output, full native/Python suites:
  unverified; not needed to assert these drafts implement no runtime changes.

## Decisions and discoveries

2026-10-16: Source inspection corrected the initial feature inventory: benchmark
comparison, rational timing, CPU/GPU resource reuse, decoder reuse, limited
effect fusion and render/encode overlap already exist. Tickets target gaps or
measured extensions rather than rebuilding them.

2026-10-16: Proposed 50 child tickets: 10 P0, 19 P1, 15 P2, six P3.
Eighteen deliver implementation or verification, 32 deliver research, prototypes
or measurement. The larger research share reflects intentionally undecided scope.

## Completion / handoff

Next action: review [tickets.md](tickets.md), confirming size, edges and any
merge/split requests. Then publish approved scope without beginning implementation.
Before merging, transfer lasting decisions into permanent docs if necessary and
remove these temporary plans per the repository lifecycle. GitHub Issues is the
authoritative backlog after publication; these drafts are not a local tracker.
