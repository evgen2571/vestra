---
name: performance-benchmarking
description: "Use when measuring Vestra rendering speed, GPU shader-pass cost, preparation latency, memory consumption, resource reuse, or performance regressions."
---

# Measure before optimizing

Start with [performance](../../../docs/development/performance.md) and
[benchmark documentation](../../../benchmarks/README.md).
Reuse existing `scripts/benchmark.py`, `benchmarks/suites.json` and
`just` recipes; do not add a second benchmark framework.

## Workflow

1. Identify the affected stage: preparation, evaluation, frame rendering,
   GPU upload/dispatch/readback, media decode, encoding or whole render.
   End-to-end speed cannot by itself prove an effect shader became faster.
2. Capture machine, revision, toolchain, FFmpeg, resolution/FPS, frames,
   cache state, actual selected backend, adapter and classification.
3. Run `just benchmark-smoke target/benchmark-results/smoke` to check
   workload viability. **Smoke timings are not a baseline.**
4. Compare repeatable canonical suites on the *same* machine:
   ```bash
   just benchmark target/benchmark-results/before
   # Apply the isolated optimization.
   just benchmark target/benchmark-results/after
   just benchmark-compare \
     target/benchmark-results/before/suite.json \
     target/benchmark-results/after/suite.json
   ```
5. For real GPU runs, use `just benchmark OUTPUT hardware-wgpu` after
   [hardware discovery](../wgpu-validation/SKILL.md). Never compare
   software-WGPU results with hardware timings as if equivalent.
6. Inspect raw sample distributions, medians, stage timings, resource
   counters, memory peaks and adaptation/fallback. Workload/environment
   mismatch requires a new baseline.
7. Add a focused effect workload when the canonical suite misses the
   operation; distinguish prepared-frame latency from one-shot/encoding
   overhead. Test 1080p cost and 4K correctness/resource limitations.
8. Re-run correctness/visual tests before accepting an optimization.

Keep generated reports in `target/benchmark-results/`, not in Git.
Never publish user-specific environment dumps or unsupported performance
claims. Report measured deltas with their actual timing scope.
