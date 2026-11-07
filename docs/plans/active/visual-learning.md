# Visual learning documentation: review pass
Status: completed
Branch / baseline: `docs/visual-learning`, reviewed `343f558f0618e29ccaaa27ab89af037827f93843`.

## Objective
Improve the teaching value and visual quality of the documentation redesign:
clear comparisons, compact diagrams, silent audio-response studies, and learning
pages whose flow is not interrupted by website-media maintenance records.

## Baseline and references
- The primary worktree was clean and exactly at the reviewed revision; there
  were no subsequent changes to reconcile. Earlier restructuring is preserved.
- Follow [documentation authoring](../../development/documentation.md) and
  [PLANS.md](../../../PLANS.md). Current code/tests govern model behavior.
- Source/preview pairing lives in `docs/assets/examples/`, the shared media
  renderer, and `docs/assets/renders/learning-media.json`.
- Motion Tile geometry is in `crates/vestra-render-common/src/geometry.rs`;
  CPU raster sampling is in `crates/vestra-render-cpu/src/cpu/raster.rs`.
  No CodeGraph index was available; sources and relevant tests were inspected.

## Scope and non-goals
Improve all affected lessons and retained demonstrations, maintain useful
concept/guide/reference/development separation and URLs/anchors, and preserve
reproducible source and credits. Do not modify the engine, public APIs, generated
parameter contracts, repository settings, or publishing state. No new runtime
dependencies, custom media player or frontend framework.

## Constraints and decisions
- Reuse exactly three task worktrees: primary `/home/evgen2571/.codex/worktrees/9296/vestra` owns effects,
  editorial cleanup, shared CSS/tooling/manifests and integration; diagram worker
  `8b2f`; audio worker `a518`. No additional trees were created.
- Source/treatment appear together on the same clock. Static previews remain
  640×520; temporal comparisons will use the same framing without the encoded footer.
  Audio studies use 640×560; all learning delivery videos contain no audio streams.
- Reuse the existing CC0 landscape as a 36 KiB crop. Bright lettering exposes light
  effects; a high-contrast moving object exposes motion blur. Input provenance
  and the crop command live beside the downloadable source in `learning-inputs.txt`.
- Preserve analysis audio while setting `output_audio=False`. Waveform/playheads,
  actual measured response curves and matched spectrum colors explain timing.
- Diagrams use the same embedded light/dark tokens, with the selected site theme
  passed through `img` color-scheme. Cap diagrams at 480 px independently of media;
  recompose rather than scale tall figures into unreadable text.
- Keep concise source/run links in lessons; consolidate generation, hashes,
  posters, encoding and backend records in contributor docs and the manifest.

## Milestones
- [x] Recompose all 12 diagrams, including visible independent authoring placements;
  integrate sizing/theme CSS and verify initial/manual theme changes.
- [x] Redesign three silent audio studies with measured curves, visible timing,
  bounded binding and useful linear/radial spectra; verify output-audio parity.
- [x] Diagnose Motion Tile gaps; make temporal and static effects show an obvious
  synchronized baseline/treatment difference at normal displayed size.
- [x] Remove maintenance-heavy lesson sections and redundant full programs;
  consolidate provenance and regeneration without losing runnable examples.
- [x] Final integrated verification, diff review and review-ready handoff.

## Acceptance criteria
Inspect delivered images and decoded timeline samples, not only encoding status.
Every retained figure must teach a specific change at desktop/mobile viewing
size. Compare identical inputs/settings/times, verify zero audio streams,
refresh posters/manifests after final renders, and test examples/downloads.
Check strict site build, source/generated links/media/snippets, formatting/lint,
generation freshness, and selected/manual themes at `/vestra/`. Preserve existing
user work and stop before pushing, opening a PR, merging or deployment.

## Progress and verification
- Integrated diagram work as `903f498` and initial audio work as `cb1e46a`.
  The audio control-band follow-up `90691ba` was integrated as a text delta;
  primary media generation recreates its delivery assets with shared settings.
- Inspected all 25 effect comparisons plus the order comparison at 320 px, and
  decoded beginning/middle/late frames of motion/audio examples. Independent
  review confirmed sources/settings/captions and found one stale description
  (procedural source vs photograph), now corrected.
- 41 focused checks passed: guide snippets, effect ownership/coverage/order,
  temporal comparisons and tile coverage, actual audio curves and spectrum
  layouts, output-audio frame parity, clear control band, media validation,
  and source/input/delivery hash freshness.
- Strict site check: 14,108 generated references, zero errors, nine tests passed.
  Source links: 713 in 116 Markdown files, zero missing.
- Effect freshness, six exporter and 19 generator tests passed. Formatting and
  lint passed for changed examples/tooling/tests and the maintained project surface.
- Browser: 33 affected pages × 390/1440 px × light/dark = 132 passing layout checks;
  no broken images, document/code overflow, oversized diagrams, wrong diagram
  color-scheme or distorted media. Paused mobile controls leave lesson content
  visible. Initial and manual theme switching were checked.
- All 12 learning MP4s were probed with exactly one video stream and no audio.
  Shared rendering verifies dimensions, decoded frame counts and exact PTS on
  originals/delivery copies. Final camera-shake tuning, rendering and manifest
  freshness are complete. All 44 browser playback/pause/seek cases passed across
  11 embedded videos, two widths and two themes.
- Final checks repeated after integration: `just site-check docs-check`, the
  41 focused tests (with `--import-mode=importlib` to distinguish the two test
  directories), Ruff check/format on changed Python files, and
  `just fmt-check lint-python gen-effects-check` all passed. Delivery stream,
  dimensions, frame count and timestamp checks were rerun for all 12 MP4s.
- Final diff review preserved the engine/API boundary and removed obsolete
  order-render branching; catalog alt text now identifies each actual input.

## Decisions and discoveries
Motion Tile has an unresolved shared-geometry anchor defect. An opaque 640×360
rectangle at scale 0.7 and centered position/anchor leaves 27.75% uncovered at both
200% and 400%: expansion changes left/up bounds but still stops at (544,306).
An origin anchor with explicit nominal-image position covers the full canvas.
The affine uses expanded dimensions without the shifted virtual origin in its
anchor calculation. Independent CPU reproduction confirmed this; no engine fix
was made. The contributor guide includes a minimal reproduction. The catalog
uses identical origin anchors on baseline/treatment and outlines nominal bounds.

## Completion / handoff
Review pass and group/layout follow-up complete; compact diagrams, silent
studies and source/media/editorial changes are ready for review. No publishing
actions were taken. Preview from the primary worktree with
`just docs-serve`; regenerate in the native environment with
`just python-sync` then `just docs-media --learning`.
CPU evidence only; WGPU was not retested. The center-anchor issue remains
explicitly documented. Remove this completed plan before any future merge.

## Follow-up: groups and catalog layout
Baseline: `a5845e0`; clean working tree, with the earlier review commits already
present on the task branch. No unrelated changes to reconcile.

- [x] Lead the setup guide with standalone `Group`, explain source snapshots vs
  owned `root.group()` placements, and centralize local coordinates/time.
- [x] Add a root/group coordinate diagram and an actual two-placement card study;
  validate the documented coordinates, timing and independence on CPU.
- [x] Restore two-column video cards and remove the encoded control footer/divider;
  regenerate the four temporal clips and their posters/manifests.
- [x] Check snippets, media freshness, links, formatting and native examples;
  inspect desktop/mobile layouts and manual light/dark switching; commit only.

Use the existing SVG theme tokens, media renderer and learning manifest. No
engine/API changes, new worktrees, dependencies, player or publishing actions.

Follow-up evidence:
- 43 focused native/media/snippet tests passed, including six exact rendered
  coordinate/time samples and snapshot/placement independence. The height check
  first failed against 640×640 output, then passed with the footer removed.
- `group_placements.py` executed as downloaded: 640×360, 90 frames, 3 seconds,
  CPU, silent H.264. Its still comes from the same project via the shared renderer.
- `just docs-media --learning` regenerated and verified original/delivery media,
  posters and source/input/delivery hashes. All 12 learning delivery videos remain
  silent; the four temporal effects now use 640×520 with no footer or divider.
- All six affected pages checked at 390/1440 px in both themes: 24 layouts passed
  with no document overflow, broken images or wrong SVG color-scheme. The four
  video cards form two desktop columns and one mobile column. All 16 video
  playback/pause cases passed. Initial appearance and manual theme switching
  were checked, plus diagram/card screenshots and decoded timeline contact sheets.
- Strict site check: 14,147 generated references, zero errors, nine site tests
  passed. Source links: 721 in 116 Markdown files, zero missing. Changed-Python
  Ruff checks/formatting and project `fmt-check`/`lint-python` passed. CPU only; no engine/API changes or new rendering issue.
  Native video controls now overlay the lower edge while visible, without an
  encoded footer; audio study layout is unchanged.

## Follow-up: reader-first examples
Baseline: `979f8da`, clean task branch. The nested-coordinate diagram still
required readers to decode repeated axes and normalized values. Replace it with
named canvases, the visible child and one explicit pixel-offset addition. Keep
geometry and timing unchanged. Review learning snippets and source programs for
blank lines between operations and readable constructor arguments, rather than
only formatting the reported snippet. Check semantic equivalence, snippets,
links/media freshness and the diagram at mobile/desktop widths in both themes.
No new media scenes, engine changes, worktrees or publishing actions.

Reader-first follow-up complete:
- Replaced repeated normalized axes with clearly named root/group/child artwork,
  a colored local canvas, and the single pixel-offset addition. Diagram geometry
  and code still describe the same unscaled, unrotated placement.
- Reviewed learning snippets and all eight downloadable programs. Separated
  setup, source/child construction, placement, animation and rendering; expanded
  crowded argument lists. Kept the quickstart source and download byte-identical.
- All 50 Python blocks in changed pages and all changed programs preserve their
  parsed syntax trees. Only source-program hashes were refreshed; input, rendered
  media, poster and original-render hashes remain unchanged. No re-encoding.
- 43 focused native/snippet/media tests passed. Strict site check: 14,347 generated
  references, zero errors, nine tests passed. Source links: 721 in 116 Markdown
  files, zero missing. Changed-Python Ruff check/format and project formatting/lint
  passed; `git diff --check` passed.
- 76 browser layouts passed: 19 pages at 390/1440 px in both manually selected
  themes. Inspected diagram and example screenshots, including the mobile code
  block; shortened a clipped comment. No document overflow, broken images or
  wrong diagram themes. Rendering semantics and engine/API behavior are unchanged.

## Follow-up: authored positions in the group diagram
Baseline: `637faca`, clean task branch. The pixel-offset diagram explains a
calculation rather than the positions readers author. Replace it with only the
actual normalized `position` values and their center anchors, remove the pixel
arithmetic and dimension labels, and shorten the figure. Update its surrounding
explanation and the incoming coordinate-page description. Keep code, geometry,
time, media and public APIs unchanged. Verify the figure at mobile/desktop widths
in both themes, current links/snippets, and the existing local-coordinate render.

Normalized-position follow-up complete:
- The figure labels only the authored group `(0.5, 0.5)` and child `(0.25, 0.5)`
  positions, with callouts to their center anchors. Removed all pixel arithmetic,
  dimensions and derived root coordinates. Reduced height from 390 to 312 pixels;
  kept 22-pixel SVG text and adequate label padding at mobile width.
- Rewrote the explanation around which parent canvas each `position` uses and
  updated the incoming coordinate-page link description. Example code unchanged.
- Strict site/link checks passed: 14,347 generated references, zero errors, nine
  tests; 721 source links in 116 Markdown files, zero missing. Existing CPU
  coordinate/time frame regression passed. Four browser layouts and screenshots
  verified at 390/1440 widths in both manually selected themes. Video regeneration
  and engine/GPU checks were unnecessary for this SVG/prose-only change.
