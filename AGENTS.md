# Agent guidance for Vestra

This is the shared project instruction file for Codex and Claude Code. Keep it
short: put only project facts, gotchas and checks that an agent would otherwise
have to rediscover. Link to the source documentation instead of copying it.

## Start here

- Confirm the current branch and worktree with `git status --short --branch`.
  Stay on the assigned branch; pull requests target `develop`.
- Read the source and tests before relying on a description of behavior. Read
  `README.md` and `CONTRIBUTING.md` when setup or contribution workflow matters.
- For substantial cross-component work, use an active plan under
  `docs/plans/active/` as described in [PLANS.md](PLANS.md). Small localized
  fixes need no plan.
- Load only the relevant repository skill from
  [.agents/skills/README.md](.agents/skills/README.md). Claude Code discovers
  the same skills through `.claude/skills/`.

## Task routing

| Task | Read first |
| --- | --- |
| Workspace and ownership | [Overview](docs/development/architecture/overview.md), [crates](docs/development/architecture/crates.md) |
| Canonical model, schema or animation | [Project to plan](docs/development/architecture/project-to-plan.md), [format](docs/reference/project-format.md) |
| Effects or CPU/WGPU parity | [Adding effects](docs/development/extending/effect.md), [effect pipeline](docs/development/architecture/effect-pipeline.md) |
| CPU renderer | [CPU renderer](docs/development/architecture/cpu-renderer.md) |
| GPU pipelines and hardware | [WGPU renderer](docs/development/architecture/wgpu-renderer.md), [validation](docs/development/gpu-validation.md) |
| FFmpeg or media I/O | [Media architecture](docs/development/architecture/media-io.md), [media pipeline](docs/development/media-pipeline.md) |
| Timeline, duration or boundaries | [Project to plan](docs/development/architecture/project-to-plan.md#time-domains-and-frame-boundaries), [testing](docs/development/testing.md#timeline-and-media-checks) |
| Python API and bindings | [Python binding architecture](docs/development/architecture/python-bindings.md) |
| Tests and CI | [Testing](docs/development/testing.md), [CI](.github/workflows/ci.yml) |
| Profiling and benchmarks | [Performance](docs/development/performance.md) |
| Publishing or licenses | [Releasing](docs/development/releasing.md), [notices](THIRD_PARTY_NOTICES.md) |

## Architecture invariants

- `vestra-core` owns the canonical project model, validation, plan
  compilation/evaluation and effect descriptors. It has no file I/O, FFmpeg or
  GPU state.
- `vestra-render` consumes evaluated work; CPU and WGPU share the same
  semantics. `vestra-media` owns media preparation, encoding and output.
  `vestra` is the public SDK, adapted by Python and the CLI.
- Preserve composition-local time, effect ordering and stage, alpha semantics,
  mask/matte behavior, resource lifetime and error reporting.
- Treat Rust/Python interfaces, canonical JSON schema, diagnostics and backend
  support as public contracts. Do not silently broaden support claims.
- Reuse existing utilities and fixtures. Avoid unrelated rewrites. If behavior
  changes, update the appropriate guide, reference or support matrix. Preserve
  licenses for third-party shaders, assets and fonts; use
  [showcase assets](examples/showcase/ASSETS.md) as the provenance pattern.

## Commands and evidence

Run from the repository root inside `nix develop` (or an equivalent environment
with Just, Rust, uv and the required native dependencies). Every command below
is defined by the checked-in Justfile, scripts, or workspace tooling.

- **Fast edit loop:** `just fmt-check`, `just lint-python`,
  `cargo test -p <crate> <filter>`, `just docs-check` and
  `just gen-effects-check` as relevant.
- **Focused Python:** `just python-test python-tests/<test>.py` syncs and builds
  the native package before running the selected test.
- **Build and format:** `just python-sync`,
  `cargo check --workspace --all-features`, `just fmt` or `just fmt-check`.
- **Contributor gate:** `just check` runs Rust formatting, effect generation,
  workspace checks, Clippy, Rust tests, schema validation/freshness and docs
  checks. It does not replace Python package tests or adapter validation.
- **Surface-specific tests:** `just lint`, `just test`, `just python-test`,
  `just schema-check`, `just effect-check` and `just examples`.
- **Rendered and adapter-dependent checks:** `just showcase-smoke` renders
  offline fixtures; `just wgpu-software` requires a usable software adapter
  and proves software correctness only; `just wgpu-hardware` requires a proven
  integrated or discrete adapter. Report unavailable GPU checks as unverified.
- **Packaging:** `just wheel-smoke` builds and verifies an installed wheel.

For renderer changes, inspect actual frames or media output and keep CPU,
software-WGPU and hardware-WGPU evidence separate. For encoded output, probe
the published file and check timestamps, audio sync and cleanup. See the
task-specific skills and [testing](docs/development/testing.md) for the
existing seams and limits.

## Workflow

- Define the contract, compile/evaluate, update renderer backends, then update
  authoring/serialization and tests as applicable. Public API, schema, CLI and
  diagnostics changes may require type stubs, schema checks and reference docs.
- Keep durable decisions in the active plan. Before merging, move lasting
  knowledge into permanent documentation and remove completed plans.
- Do not push or publish unless explicitly requested. Use commits formatted as
  `type: short description` without phase numbers.
- At handoff, state the files changed, checks run, limitations and any
  unverified work.

## Repository-specific references

- Issue and spec workflow: [issue tracker](docs/agents/issue-tracker.md) and
  [triage labels](docs/agents/triage-labels.md).
- Domain vocabulary and ADR guidance: [domain docs](docs/agents/domain.md).
