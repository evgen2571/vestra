# Documentation learning and navigation
Status: in progress
Branch / baseline: docs/learning-navigation / develop 38caed6822d7d24aba9ea1de20bf61a8114ccc5c

## Objective
Carry the reviewed website forward with discoverable technical documentation,
readable examples, text identity and credits beside the examples that use assets.
The implementation request in this session is the specification.

## Baseline and references
Fetched origin on 2026-11-06: develop remains 38caed6; docs/zensical-site remains
a958fd3. Working tree was clean. The website is transferred as a cumulative patch,
excluding docs/assets/logo.svg, without importing its documentation commits.
See PLANS.md, docs/development/documentation.md, zensical.toml, the gallery and
examples/showcase sources. CodeGraph is not indexed for this worktree.

## Scope and non-goals
Navigation, introductory next steps, showcased/downloadable source readability,
source/version links, credit placement, theme identity and relevant checks.
No engine/API changes, README redesign, new assets/dependencies, publishing,
remote mutations or broad teaching rewrite.

## Constraints and decisions
- Keep existing tutorial/page URLs and heading anchors where useful.
- Expose existing accurate guides/concepts/reference rather than placeholders.
- Preserve CLI arguments and scene semantics; compare serialized projects before
  and after cleanup. Only changed rendering behavior requires regenerated media.
- Commit cleaned source before pinning gallery links to it. The same checkout
  contains source, inputs and original previews; no separate branch dependency.
- Keep license files and distribution/rebuild routes intact. Credits live beside
  examples; documentation authoring owns the convention.
- Explicitly suppress theme logo and browser favicon fallbacks.

## Milestones
- [x] Transfer website with no custom logo in any new commit.
- [x] Clean example sources and relocate input credits; commit reviewable source revision.
- [ ] Organize navigation, update links and next steps, add focused regression checks.
- [ ] Run source/docs/style/site gates and desktop/mobile browser inspection;
      review complete diff and commit final corrections.

## Acceptance criteria
Beginner sequence remains intact. Shapes, groups, masks, effects, transitions,
audio and spectra have visible topic navigation. Gallery pages have sidebars.
Contributor entry includes development and documentation guidance. Source/setup/
preview revision agrees. Required license copies remain packaged. Generated
links work beneath /vestra/. Desktop/mobile, both themes, keyboard, overflow,
previous/next and identity have actual browser evidence. All new commits omit
the custom logo; existing branches remain intact.

## Progress and verification
- Repository and latest remote branches inspected; clean transfer applied.
- Identified credit callers, package license include rules and wheel assertions.
- Baseline `just site-check`: strict build passed, 3,977 generated references
  valid, 5 tests passed; logo file/config absent and fallbacks suppressed.
- Source examples currently have indexed settings and sparse timing explanation;
  native availability and serialization comparison still to verify.

## Decisions and discoveries
- 2026-11-06: latest origin contains no subsequent changes to either baseline.
- 2026-11-06: use separate navigation sections with the existing page paths.

## Completion / handoff
Pending implementation and verification. Remove this completed plan before the
final handoff; durable authoring decisions belong in the authoring guide.

- Source cleanup: all five canonical projects at full and smoke settings match
  the captured baseline (floats rounded to 12 decimal places). Native extension
  reused from the develop checkout; engine/API code is unchanged. Media retained.
- `just showcase-style` passed; Commons/Flickr/CC0 terms checked directly on
  2026-11-06. Required license files and packaging includes remain unchanged.
