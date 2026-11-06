# Readable documentation examples
Status: in progress
Branch / baseline: docs/learning-navigation / 6d9e545

## Objective
Make user-facing examples read like small programs written to teach one task.
The user's mask snippet demonstrates the problem: unrelated features, vague
names, dense constructors and redundant settings presented together.

## Baseline and references
Working tree clean, assigned branch tracks origin/docs/learning-navigation.
Follow AGENTS.md, PLANS.md and docs/development/documentation.md. CodeGraph was
not indexed in the preceding iteration; use direct inspection. Native develop
checkout can supply the unchanged engine for authoring/snapshot checks.

## Scope and non-goals
Audit documentation code blocks, downloadable tutorials, examples/python,
showcase scenes, canonical JSON examples and code-to-video sources. Improve
organization, names, explicit settings, prerequisites and selective explanations.
Preserve CLI arguments, APIs, diagnostic behavior, scene output, licenses and
logo-free branch history. No engine changes, dependencies or shared example framework.
Keep advanced reference contracts accurate rather than removing necessary detail.

## Constraints and decisions
- One concept at a time, with prerequisites made visible and variations labeled.
- Comments explain timing, ownership, units or decisions; prose carries teaching.
- Prefer direct imports and descriptive variables to repeated namespaces and
  indexed structures when they obscure the code. Keep genuinely simple snippets simple.
- Compare canonical projects before/after cleanup; retain unchanged media.
- Commit cleaned showcased source before updating pinned gallery/setup links.
- Introductory quickstart and download remain identical.

## Milestones
- [x] Inventory/read all user-facing examples and capture source scene baselines.
- [x] Simplify guide/reference snippets, starting with masks; preserve anchors.
- [x] Clean standalone/showcase sources and JSON presentation without changing output.
- [ ] Verify snippet authoring, source equivalence, consistency, style and site;
      inspect representative pages; review diff and make local commits.

## Acceptance criteria
The mask guide introduces one understandable operation per step. Readers can
identify setup, content, animation/effects and output without dense chains or
comment walls. Existing scripts/CLI and technical contracts remain valid. All
showcased source links point to the cleaned revision. Site links/build and
relevant documentation/Python checks pass. No push or publication.

## Progress and verification
Reviewed all user-facing Python sources, documentation snippets, canonical JSON
and the advanced helper/diagnostic entry points. Captured 22 standalone projects,
10 full/smoke gallery configurations and 13 stylized looks. Canonical comparisons
still match (round floats to 12 decimals and normalize temporary directory names).
JSON data is identical after expanding dense formatting.

New tests execute all snippets from all ten Python guides through the public API;
10 pass. A syntax test covers all Python documentation blocks and included files.
The guide execution caught an unsupported draft EffectStack.clear call (removed)
and an existing too-small group budget (corrected to include the project canvas).
The revised mask guide rendered 1920×1080/30fps/five seconds on CPU using a
converted PNG from the bundled CC0 landscape. Probe and frame inspection pending.

## Decisions and discoveries
The previous pass focused on gallery navigation and five sources; this request
explicitly broadens readability work to the rest of the example surfaces.

## Completion / handoff
Pending. Remove this plan when complete; retain the example-writing convention
in documentation authoring guidance.
