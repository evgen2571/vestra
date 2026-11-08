# Release artifacts implementation plan

Status: in progress
Branch / baseline: `feat/release-artifacts`, latest `origin/develop` at
`40bc8b779e4e76063f87fe00799302259aa0039c`.

## Objective

Implement [issue #107](https://github.com/evgen2571/vestra/issues/107): fewer
Python builds, independently downloadable native CLI, and validated artifacts
prepared before public publication.

## Baseline and references

The existing release workflow runs on an already published release and builds
20 interpreter-specific wheels, 20 FFmpeg source archives and one sdist.
CI's native checks run on Linux; Windows/macOS only test benchmark tooling.
See `.github/workflows/{ci,release}.yml`, `docs/development/releasing.md`,
`scripts/{verify-wheel,archive-ffmpeg-source}.py` and `crates/vestra-cli`.

## Scope and non-goals

Change packaging configuration, release/CI workflows, artifact helpers, their
tests and installation/maintainer docs. Preserve engine behavior, supported
CPython 3.11–3.14, five platform targets, external ffmpeg/ffprobe requirements,
OIDC and protected environment approval. Do not merge or publish a release.

## Constraints and decisions

- Investigate `abi3-py311` with real builds and boundary measurements before
  claiming compatibility. Build once per target and validate the same wheel
  on each supported interpreter in fresh environments.
- Use a reusable artifact workflow for PR packaging checks and release
  preparation. Keep early native CI lightweight and path-scoped.
- Build CLI separately from Python using Cargo. Keep wheel and CLI target
  directories distinct so corresponding FFmpeg source cannot be misassociated.
- Manual runs prepare candidates without publication. Tags prepare exact
  revisions; publication follows validation and environment approval. Keep a
  draft until GitHub artifacts and PyPI publication have succeeded.
- Preserve corresponding source and licenses per artifact; do not assume a
  source ZIP resolves LGPL static-link relinking obligations. Document review.

## Milestones

- [x] Python ABI: compile, installed-wheel smoke and measured baseline comparison.
- [x] CLI archives: five target recipe, checksums, packaged-binary validation.
- [x] FFmpeg source: explicit discovery roots, artifact/revision association,
      fail-closed ambiguity tests and compliance documentation.
- [ ] CI/release: immutable preparation, clean interpreter/platform smoke,
      required aggregate gate and controlled publication.
- [ ] Delivery: documentation, focused/full relevant checks, complete diff
      review, commits, pushed branch and one PR to develop with passing CI.

## Acceptance criteria

The issue's full acceptance checklist is authoritative. Evidence includes five
wheel builds (if ABI viable), 20 same-wheel interpreter smoke jobs, five CLI
packages with source/checksum smoke, tested source-selection failure cases,
style/docs gates, earlier Windows/macOS native coverage, and a reviewed PR.

## Progress and verification

- Read issue, inspected clean checkout, fetched origin/develop and created an
  isolated worktree. CodeGraph is unavailable (repository is not indexed).
- Parallel independent tasks own ABI/wheel smoke, CLI helpers and FFmpeg source;
  coordinator owns workflows, docs, integration and review.
- Baseline archive/docs tests passed (5). Final release-helper checks pass (41),
  broad scripts pass (201, 13 skips, 19 subtests), style passes, docs links/site
  build/generated links/site tests pass. Actionlint and whitespace checks pass.
- Same abi3 wheel smoke passed on CPython 3.11.15, 3.12.13, 3.13.14, 3.14.6;
  abi3audit found no mismatches/violations. Local platform evidence is Linux x64.
- Local CLI release package, checksum, extraction, validation and CPU MP4 probe
  passed; inspected red/blue frames. Build reused matching FFmpeg source/cache.
- Warm binding rebuild: 37.83s baseline / 36.08s abi3 (one noisy pair, no speedup
  claim). Wheel size 21,842,950 / 21,845,777 bytes (+0.013%). Three alternating
  benchmark pairs show 16-layer from_dict 261.14 / 288.02 microseconds (+10.3%);
  other tiny boundary calls are noisy. No renderer-throughput claim.
- Independent review found a retry/source-overwrite hazard. Stage now checks
  existing PyPI and GitHub artifact hashes before replacing any staged files;
  regression test passed. Platform Actions verification remains outstanding.

## Decisions and discoveries

- User authorized implementation decisions, delegation, pushing and opening a
  PR; no extra design approval round is needed.
- Separate FFmpeg source archives avoid unproven deduplication across builds.

## Completion / handoff

Pending implementation and verification. No release publication or merge is
part of this task.
