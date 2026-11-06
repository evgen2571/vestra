# Visual learning documentation
Status: completed
Branch / baseline: `docs/visual-learning`, remote `develop` at `3e77c57`.

## Objective
Continue the documentation redesign after PR #101: help a developer understand,
predict and adapt sources, coordinates, groups, time, masks, lifecycle,
transitions, effects, and audio-driven visuals while building a result.
The user brief is the task specification; engine code and tests govern behavior.

## Baseline and references
- Clean initial worktree; `git fetch origin develop` confirmed the baseline.
  At handoff, `git ls-remote --heads origin` no longer advertises `develop`, and
  refetching it fails. Retain the initial fetched baseline; no remote changes
  were made by this task.
- Follow [documentation authoring](../../development/documentation.md),
  [the planning convention](../../../PLANS.md), and existing native site tooling.
- Existing guides mix usage, contracts and implementation; effects guide is 417
  lines and effects reference 393 lines. Parameter reference is generated.
- No CodeGraph index is present; inspect Python authoring, Rust evaluation and
  relevant tests directly. Baseline strict site checks passed.

## Scope and non-goals
Rewrite the affected learning path, preserve useful contracts and URLs/anchors,
add schematic SVGs and actual CPU-rendered comparisons with reproducible source.
No engine/API behavior changes, dependencies, custom frontend, deployment,
pushes, PRs, merges or repository settings changes.

## Constraints and decisions
- At most three task worktrees: primary (effects, integration, media tooling,
  navigation and verification), concepts worker, audio/transitions worker.
- Shared navigation/CSS/tooling belong to primary. Workers own disjoint pages,
  example programs and named diagram assets; integrate commits on primary.
- Keep current site identity, native media controls and lightweight static
  catalog previews. Document CPU evidence separately from backend support.
- Use original procedural scenes and bundled fonts; keep QA originals in
  ignored `target/`, web assets in `docs/assets/`, and provenance beside source.
- Preserve generated parameter source of truth; generation changes only if
  presentation requires them. No need to change mathematical behavior.

## Milestones
- [x] Concepts: sources/placements, coordinates/anchors, nested groups, time,
  masks/mattes and lifecycle form a linked path with legible schematic diagrams;
  preserve and relocate detailed knowledge; verify snippets against code/tests.
- [x] Transitions: four representative families on equal inputs/timing, overlap
  diagram, normalized progress, composition ownership, endpoints and errors;
  complete runnable source and real renders inspected.
- [x] Effects: focused basic guide and processing concept; browsable grouped
  catalog covering most informative public effects; order comparison; palette
  and ASCII guides; precise contracts and implementation math remain accessible.
- [x] Audio: clip→analysis→signal→binding/spectrum relationship; bounded response,
  attack/release comparison and linear/radial spectra with audible original audio.
- [x] Integration: navigation, incoming links/anchors, regeneration instructions,
  provenance and examples agree; compact assets; meaningful code checks.
- [x] Finished experience: strict build, link/media/snippet/freshness checks,
  native example execution, actual frame inspection, desktop/mobile in both
  themes, keyboard/media checks and `/vestra/` path verification; commits ready.

## Acceptance criteria
Every requested topic has an understandable reading order, prerequisites,
expected observations, runnable/adaptable code and a route to exact contracts.
Rendered claims come from actual Vestra; schematics are identified. Comparisons
use controlled settings. No generated reference is hand edited. Site works
without a native build, and includes/downloads/previews match source. Changed
code is linted/formatted and regression checks cover nontrivial tooling.
Unavailable checks must be recorded, never represented as passing.

## Progress and verification
- Primary isolated worktree: `/home/evgen2571/.codex/worktrees/9296/vestra`.
  Native environment rebuilt through Nix/uv with Python 3.13.
- Integrated concepts `ecb4e6d`, audio/transitions `59aab74`, effects/catalog and
  shared tooling `d4b2040`. Exactly three task worktrees; no pushes or publishing.
- Strict `just site-check`: 14,374 generated links/media references, zero errors;
  nine site/navigation/snippet/freshness tests passed. `/vestra/` checked by
  generated checker and actual browser navigation.
- `just docs-check`: 740 source links in 116 Markdown files, zero missing.
- Focused native checks: 29 passed (guide snippets including full downloads,
  every catalog effect, order-dependent pixels, source/settings consistency,
  audio pulse input and actual smoothing behavior, transition fit/conflicts,
  media verifier failures). Final ownership assertions also passed all 16 guides.
- `just gen-effects-check`: freshness, six exporter and 19 generator tests pass.
  Generated parameters remain unchanged. `just fmt-check`, `just lint-python`,
  Ruff checks/format for all changed Python examples/tooling/tests pass.
- `just docs-media --learning`: four maintained programs, 14 CPU videos, actual
  frame counts/timestamps, dimensions, duration, AAC presence verified on originals
  and delivery files. Stills, before/middle/after video contact sheets inspected.
  Decoded AAC on all three audio studies retains pulses at .25/.85/1.45/2.05/2.65s.
- Original three-example media path rerun into ignored delivery directory; all
  three videos and previews pass probing. New effects, palette, ASCII and recipe
  guide programs also executed with actual CPU encoding.
- Chromium: 12 key pages × 390/1440 px × light/dark = 48 layout checks; no broken
  images, overflowing page/code containers, missing controls or media errors.
  Diagram screenshots inspected. Native keyboard play/pause/seek, skip link,
  mobile drawer navigation and visible focus verified. All 13 embedded videos
  passed play/pause/seek at both widths/themes (52 cases) on a range-capable
  static server, mounted at `/vestra/`. Native Zensical/Python preview servers
  return HTTP 200 to Range requests; Chromium resets silent-video seeks there.
  The contributor guide records this preview limitation.
- Independent effects review found four issues; corrected RadialBlur zoom-sampling
  wording, delayed CameraShake clock, restored canonical animation reference,
  and avoided stale hashes when motion posters replace static frames.
- Broader existing native selection: 115 passed, 29 failed with exactly
  `WGPU device_request failed: Parent device is lost`; 27 mask cases, standalone
  group WGPU sequence, and high-level WGPU lowering. Full log retained at
  `/tmp/vestra-concepts-native-verification.log`. No hardware WGPU claim.
- No diff under `crates/`, `python/` or `schemas/`; engine and APIs unchanged.

## Decisions and discoveries
- 2026-11-06: Retain site architecture and existing URL pages; reorganize content
  by reader purpose. New catalog supplements, rather than replaces, parameters.
- User explicitly requests implementation and independent routine decisions;
  proceed without additional design approval gates.

- Group mask with a solid `Color` child currently fails `VESTRA-SOLID-TRANSFORM`;
  comparison uses a filled Rectangle. Limitation documented, engine unchanged.
- CPU procedural source originally exceeded the 64-active-layer preparation cap;
  reduced source layers, preserving a detailed ramp, geometric edges and text.

## Completion / handoff
Ready for local review on `docs/visual-learning`. Preview from the primary
worktree with `just docs-serve`; regenerate with `just docs-media --learning`.
The connected concept path is followed by practical effects/palette/ASCII,
transition, audio-response and spectrum guides. Exact interpolation contracts
live in reference; effect numerical algorithms live in development docs.
Nine schematics and CPU previews are reproducible from original procedural inputs.
Media manifest records program hashes, original checksums, settings and posters.

Remaining limits: WGPU verification above failed at device initialization;
new previews establish CPU behavior only. Existing Group-mask Color limitation
is documented with the rectangle workaround. Broad architecture/performance
cleanup, exhaustive effect-parameter combinations and publishing remain outside
this brief. Retain this plan for review, remove before any future merge.
