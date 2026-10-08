# Vestra repository skills

These five repository-local skills supplement [AGENTS.md](../../AGENTS.md),
[PLANS.md](../../PLANS.md) and the [development docs](../../docs/development/README.md).
They do not override the repository's existing architectural contracts.

| Skill | Activate when |
| --- | --- |
| [effect-development](effect-development/SKILL.md) | Implementing or changing effects and shader pipelines |
| [visual-regression](visual-regression/SKILL.md) | Checking actual output frames, parity or visual fidelity |
| [wgpu-validation](wgpu-validation/SKILL.md) | Debugging GPU adapters, kernels, or resource limits |
| [performance-benchmarking](performance-benchmarking/SKILL.md) | Profiling and validating renderer optimizations |
| [api-contract-sync](api-contract-sync/SKILL.md) | Updating Rust/Python/JSON contracts and schema |

Load only the skill relevant to the task. Prefer existing `just` recipes
and scripts. The visual-regression skill includes two runnable Pillow-based
tools and four local self-tests. Pillow is an isolated development dependency
for those tools, not a Vestra package/runtime dependency.
