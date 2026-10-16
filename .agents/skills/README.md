# Vestra repository skills

These seven repository-local skills supplement [AGENTS.md](../../AGENTS.md),
[PLANS.md](../../PLANS.md) and the [development docs](../../docs/development/README.md).
They do not override the repository's existing architectural contracts.

| Skill | Activate when |
| --- | --- |
| [effect-development](effect-development/SKILL.md) | Implementing or changing effects and shader pipelines |
| [visual-regression](visual-regression/SKILL.md) | Checking actual output frames, parity or visual fidelity |
| [wgpu-validation](wgpu-validation/SKILL.md) | Debugging GPU adapters, kernels, or resource limits |
| [performance-benchmarking](performance-benchmarking/SKILL.md) | Profiling and validating renderer optimizations |
| [api-contract-sync](api-contract-sync/SKILL.md) | Updating Rust/Python/JSON contracts and schema |
| [timeline-validation](timeline-validation/SKILL.md) | Diagnosing duration, nested/source time, transitions or frame boundaries |
| [media-output-validation](media-output-validation/SKILL.md) | Verifying encoded output, AV sync, publication or media failure cleanup |

Load only the skill relevant to the task. Prefer existing `just` recipes
and scripts. The visual-regression skill includes two runnable Pillow-based
tools and four local self-tests. Pillow is an isolated development dependency
for those tools, not a Vestra package/runtime dependency.

For metadata and links, inspect each `SKILL.md` frontmatter (`name` matches the
folder; `description` identifies its task) and resolve its relative targets.
The public `just docs-check` gate does not traverse `.agents/`. Keep skill
scripts unchanged unless the task actually needs different tooling.

## Shared engineering guidance

Skillshare manages shared skills in `evgen2571/skills`, outside this repository.
Use `rust-skills` selectively for the Rust topic at hand; repository contracts,
ownership, numeric/alpha semantics and measured performance take precedence.
Use `codebase-design` for justified module/seam design, `domain-modeling` when
changing domain vocabulary or ADRs, and `grilling` when an interview/stress-test
of unresolved decisions is requested. Routine fixes do not need all four.
Do not copy their rule collections here or assume their installation on another
machine. Check with `skillshare list`; install missing skills in the shared
collection and sync using that repository's conventions.
