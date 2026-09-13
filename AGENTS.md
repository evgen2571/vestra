# Vestra agent instructions

Working instructions for GPT-6 Astra in this repository. Vestra is a Rust video
editing and rendering engine with a public SDK, CLI, and Python bindings.

## Working approach

- Carry authorized work through implementation and verification. Preserve the
  full requested outcome; a passing subset is not completion.
- Inspect the worktree, branch, and relevant current code before acting. Preserve
  unrelated changes. Historical plans and prior chat are leads, not proof.
- Make routine, reversible decisions without repeatedly asking for permission.
  Ask when missing information prevents a sound decision, and continue any
  independent work while waiting.
- Keep changes proportional. Reuse existing mechanisms and prefer direct code
  over speculative abstractions, new frameworks, or duplicated engine logic.
- Give concise updates when findings or the next action change. At completion,
  state the result, verification evidence, and remaining limitations.
- Retain and poll actual handles for long-running commands. An observation
  timeout is not a failed job. Do not restart based only on a stale log.

## Finding code

<!-- CODEGRAPH_START -->
If `.codegraph/` exists at the repository root, use CodeGraph before text search
or file reads when locating or understanding code:

- Use the `codegraph_explore` MCP tool when available, including deferred tool
  discovery if needed. Name symbols or files to retrieve source and call paths.
- Otherwise use `codegraph explore "<symbols or question>"` in the shell.

If there is no index, skip CodeGraph. Do not create one without a user request.
Use `rg` and `rg --files`, then read relevant source and tests. Scope searches
and avoid dumping whole subsystems into context.
<!-- CODEGRAPH_END -->

## Ownership and dependencies

Confirm ownership in current code. Preserve these boundaries:

| Crate | Responsibility |
| --- | --- |
| `vestra-core` | Canonical model, semantic validation, compilation and evaluation |
| `vestra-render` | Rendering contracts, CPU/WGPU backends, pixels and resources |
| `vestra-media` | FFmpeg, probing/decoding, audio execution, encoding and publication |
| `vestra` | Public SDK and engine orchestration |
| `vestra-progress` | Render lifecycle events and terminal progress |
| `vestra-observability` | Application-boundary tracing configuration and output |
| `vestra-cli` | CLI parsing and presentation |
| `vestra-python` | Python conversion and bindings to the SDK |

Core semantics remain renderer-independent. Core does not acquire devices, read
assets, or invoke FFmpeg. Renderers consume compiled/evaluated semantics instead
of reinterpreting project defaults or timeline rules. CLI and Python call engine
APIs rather than recreating validation, preparation, or rendering flow. Keep
semantic validation separate from asset/render-target preflight.

## Correctness and compatibility

- Treat time/frame/sample conversions, rounding, interval endpoints, and
  zero-length behavior as correctness-sensitive. Test the relevant boundaries.
- Preserve public Rust/Python APIs, canonical JSON and generated schemas, CLI
  contracts, diagnostics, lifecycle events, and report formats unless changing
  them is part of the task.
- Trace a feature through its affected layers. Source/effect/transition changes
  may require model, compiler/evaluator, CPU, WGPU, Python lowering, schema, and
  documentation updates in the same delivery.
- If Rust changes affect Python, update and verify the Python API and stubs too.
- Reproduce bugs before fixing them. Correct the cause instead of adding guards
  that hide invalid internal state. Concentrate input validation at boundaries.
- Preserve cancellation, encoder finalization, and output publication. Rendering
  the last frame does not mean the output has been published.

## Performance work

Use the measurement foundation in [Performance](docs/development/performance.md).
Reuse existing benchmarks, timing reports, and resource counters.

```bash
python scripts/benchmark.py run --suite canonical --output target/benchmark-results/before
# Implement the optimization.
python scripts/benchmark.py run --suite canonical --output target/benchmark-results/after
python scripts/benchmark.py compare target/benchmark-results/before/suite.json target/benchmark-results/after/suite.json
```

Use a new output directory for each run. Keep source unchanged during a suite.
Smoke runs prove execution, not performance. Measure release builds on comparable
environments; inspect raw samples and resource tradeoffs before keeping a change.
Distinguish preparation, prepared-frame work, decoding, rendering, and encoding.
Do not compare a one-shot render with a prepared operation.

Avoid avoidable allocation and I/O in frame loops. Keep benchmark recording
separate from live progress/ETA. Detailed tracing and profiling remain opt-in
and run separately from baseline timing. Preserve checked-in baselines; add a
new named baseline when the workload definition changes.

## Hardware WGPU policy

- CPU, software WGPU, and hardware WGPU are separate validation classes.
- Never describe llvmpipe, Lavapipe, SwiftShader, software Vulkan, or another CPU
  adapter as hardware-GPU rendering or performance.
- GPU correctness or performance work requires hardware validation unless the
  task explicitly permits software WGPU. Prefer an integrated/discrete adapter
  and record the actual selected adapter, graphics backend, and driver.
- Never silently fall back when hardware WGPU was requested. If hardware is
  unavailable, report the hardware-GPU portion as blocked. CPU/software checks
  may continue separately but cannot satisfy the hardware requirement.
- CPU/WGPU parity requires execution evidence for affected semantics. Matching
  dispatch code or a successful build does not prove parity.

Inside WSL, inspect graphics paths before WGPU tests:

```bash
nvidia-smi
glxinfo -B
vulkaninfo --summary
```

Follow [GPU validation](docs/development/gpu-validation.md), including adapter
selection and `scripts/verify-wgpu.sh --hardware` when applicable. Do not assume
hardware availability or unavailability from a previous session.

## Verification and delivery

Start with focused checks that exercise changed behavior. Add regression tests
for meaningful failure modes; avoid tests that merely mirror source text. Verify
real renders and output properties when rendering/media behavior changes. Do
not claim success from compilation or a skipped/gated test alone.

Use [Testing](docs/development/testing.md) for commands and native environment
setup. `./scripts/check.sh` is the broader repository gate. Schema, CLI, Python,
and GPU changes have additional checks described there. Keep hardware
requirements explicit when choosing what to run.

Before delivery, inspect the final diff, run formatting and relevant checks,
and audit the result against the original request. Distinguish passing checks,
pre-existing failures, unavailable checks, and unfinished work. Update current
documentation when behavior changes.

When committing or pushing is authorized, inspect branch/remote state, include
only intended changes, and verify the resulting commit and remote ref. Never
force-push or discard unrelated work without explicit authorization.
