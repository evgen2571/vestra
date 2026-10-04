# Benchmarks

The suite definition is in `suites.json`; representative projects are in
`projects/`. The runner, Rust benchmark and comparison tests are maintained
with the engine. Generated results belong under the ignored
`target/benchmark-results/` directory.

Capture a baseline on the machine where you will measure the candidate. The
repository keeps suite definitions, projects and comparison tooling; personal
machine captures are not distributed as performance targets.

```bash
just benchmark-smoke target/benchmark-results/smoke
just benchmark target/benchmark-results/before
# Make one change and leave sources unchanged for the complete suite.
just benchmark target/benchmark-results/after
just benchmark-compare target/benchmark-results/before/suite.json target/benchmark-results/after/suite.json
```

Use a new directory for every run. The canonical CPU suite uses 1280×720,
one warmup and five samples per workload. Hardware WGPU needs a separate
baseline and confirmed hardware selection for every sample. Smoke verifies
execution and is not performance evidence.

The comparison rejects incompatible workload identities, definitions,
environments and backend selections. Preserve the original local baseline;
create a new named baseline when workloads or the environment change.

Reports include machine, toolchain, revision and executable metadata to make
local comparisons meaningful. Inspect and sanitize them before sharing, and
avoid committing home paths, environment dumps or workstation descriptions.
See [performance methodology](../docs/development/performance.md).
