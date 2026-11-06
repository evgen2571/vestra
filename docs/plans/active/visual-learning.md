# Visual learning documentation
Status: in progress
Branch / baseline: `docs/visual-learning`, remote `develop` at `3e77c57`.

## Objective
Continue the documentation redesign after PR #101: help a developer understand,
predict and adapt sources, coordinates, groups, time, masks, lifecycle,
transitions, effects, and audio-driven visuals while building a result.
The user brief is the task specification; engine code and tests govern behavior.

## Baseline and references
- Clean initial worktree; `git fetch origin develop` confirms the baseline.
- Follow [documentation authoring](../../development/documentation.md),
  [the planning convention](../../../PLANS.md), and existing native site tooling.
- Existing guides mix usage, contracts and implementation; effects guide is 417
  lines and effects reference 393 lines. Parameter reference is generated.
- No CodeGraph index is present; inspect Python authoring, Rust evaluation and
  relevant tests directly. Site baseline check is running.

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
- [ ] Transitions: four representative families on equal inputs/timing, overlap
  diagram, normalized progress, composition ownership, endpoints and errors;
  complete runnable source and real renders inspected.
- [ ] Effects: focused basic guide and processing concept; browsable grouped
  catalog covering most informative public effects; order comparison; palette
  and ASCII guides; precise contracts and implementation math remain accessible.
- [ ] Audio: clip→analysis→signal→binding/spectrum relationship; bounded response,
  attack/release comparison and linear/radial spectra with audible original audio.
- [ ] Integration: navigation, incoming links/anchors, regeneration instructions,
  provenance and examples agree; compact assets; meaningful code checks.
- [ ] Finished experience: strict build, link/media/snippet/freshness checks,
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
- 2026-11-06: Read brief, repository instructions, planning and authoring docs;
  inspected current navigation, guides, media generation and snippet tests.
- Primary isolated worktree: `/home/evgen2571/.codex/worktrees/9296/vestra`.

- Baseline `just site-check`: strict build, 11,479 generated references, 8 tests passed.
- Integrated concepts worker commit `7dab3ae` as `ecb4e6d`: three new concepts,
  six SVGs, guide/lifecycle rewrites, actual CPU 640×480 mask comparison.
- Primary native environment rebuilt with `nix develop` and uv Python 3.13.
- Root effects draft: 23 canonical effects + PseudoAscii + second dither;
  all static CPU previews rendered; six 2-second motion/baseline videos encoded.
  Contact sheets inspected; corrected Contrast/Saturation factor settings.
- 18 native guide/effect checks passed after expanding included sources; four
  media verifier tests passed (real tiny FFmpeg outputs, rejects wrong contracts).
- `just gen-effects-check` passed: freshness, 6 exporter and 19 generator tests.
- Concepts worker broader existing native checks: 115 passed, 29 WGPU failures
  (`device_request: Parent device is lost`). CPU preview evidence is independent.
- Root effects read-only review is running; audio/transitions worker is finishing
  mobile label placement and regeneration. Final browser and integrated gates pending.

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
Pending implementation and verification. Keep plan current until handoff;
remove completed plan before any future merge as required by repository policy.
