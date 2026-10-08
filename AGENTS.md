# Agent guidance for Vestra

This file applies to the whole repository. Treat it as a compact navigation
and working contract, not as a duplicate of architecture documentation.

## Begin work

1. Read `README.md`, `CONTRIBUTING.md`, and task-specific material below.
2. Read the actual source/tests before relying on a description of behavior.
3. For substantial cross-component work use [PLANS.md](PLANS.md) and update
   the active execution plan. Small localized fixes need no plan.
4. Verify the current branch, environment and relevant test commands.

## Task routing

| Task | Read first |
| --- | --- |
| Workspace and ownership | [Overview](docs/development/architecture/overview.md), [crates](docs/development/architecture/crates.md) |
| Canonical model / schema / animation | [Project to plan](docs/development/architecture/project-to-plan.md), [format](docs/reference/project-format.md) |
| Effects / WGSL / CPU-WGPU parity | [Adding effects](docs/development/extending/effect.md), [effect pipeline](docs/development/architecture/effect-pipeline.md) |
| CPU renderer | [CPU renderer](docs/development/architecture/cpu-renderer.md) |
| GPU pipelines and hardware | [WGPU renderer](docs/development/architecture/wgpu-renderer.md), [validation](docs/development/gpu-validation.md) |
| FFmpeg / media I/O | [Media architecture](docs/development/architecture/media-io.md), [media pipeline](docs/development/media-pipeline.md) |
| Python API and bindings | [Python binding architecture](docs/development/architecture/python-bindings.md) |
| Tests and CI | [Testing](docs/development/testing.md) |
| Profiling and benchmarks | [Performance](docs/development/performance.md) |
| Publishing / licenses | [Releasing](docs/development/releasing.md), [notices](THIRD_PARTY_NOTICES.md) |

## Architecture invariants

- `vestra-core` owns the canonical project model, validation, plan
  compilation/evaluation and effect descriptors. No file I/O, FFmpeg or
  GPU state in core.
- `vestra-render` consumes evaluated work; CPU and WGPU do not create
  separate semantics. `vestra-media` owns media preparation/encoding/output.
  `vestra` is the public SDK, adapted by Python/CLI.
- Preserve composition-local time, effect ordering and stage, alpha
  semantics, mask/matte behavior, resource lifetime and error reporting.
- Treat Rust/Python interfaces, canonical JSON schema, diagnostics and backend
  support as public contracts. Do not silently broaden support claims.
- Reuse existing utilities/fixtures. Avoid unrelated architectural rewrites.
  If behavior changes, update the appropriate guide, reference and support
  matrix. Preserve licenses for third-party shaders, assets and fonts;
  use [showcase assets](examples/showcase/ASSETS.md) as a provenance pattern.

## Workflow and checks

- Stay on the assigned branch. Never push to `main` unless requested.
- For new features: define contract -> compile/evaluate -> renderer(s) ->
  authoring/serialization -> tests -> user docs/examples.
- Keep durable decisions, progress and blockers in the active plan where
  required by [PLANS.md](PLANS.md).
- Use `nix develop` or an equivalent native environment. Start with focused
  `cargo test -p <crate> <filter>` / focused Python tests; finish with the
  relevant `just check`, `just python-test` and `just docs-check`.
- GPU changes also require `just wgpu-software` and, when hardware is
  available, `just wgpu-hardware`. A parsed shader or software adapter does
  not prove rendering quality or hardware parity. Inspect rendered frames.
  Report tests that could not run as **unverified**.
- Use meaningful commits formatted `type: short description` without phase
  numbers. At handoff state changes, tests/results, limitations and next work.
