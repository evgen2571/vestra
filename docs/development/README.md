<span id="vestra-development"></span>

# Contributing to Vestra

Use the [contribution guide](https://github.com/evgen2571/vestra/blob/develop/CONTRIBUTING.md)
to set up a checkout and prepare a pull request against `develop`. Then choose
the route that matches your change.

<span id="testing-and-performance"></span>
<span id="architecture"></span>
<span id="release-procedures"></span>

## Choose a task

| I want to… | Start here | Check the result |
| --- | --- | --- |
| Improve documentation or examples | [Documentation authoring](documentation.md); no native build needed for site edits | `just docs-check`, `just site-check` and browser inspection; execute changed examples. |
| Fix a bug | [Architecture overview](architecture/overview.md), then [crate ownership](architecture/crates.md) | Reproduce it with a focused test; use the [check chooser](testing.md#choose-checks-for-your-change). |
| Add a source, effect, transition or backend | Choose an [extension guide](#extension-guides) below | Verify affected contracts and backends with [Testing](testing.md). |
| Investigate rendering | [Render pipeline](architecture/render-pipeline.md) and [GPU validation](gpu-validation.md) | Inspect frames and report the actual backend and adapter. |
| Measure or improve speed | [Performance](performance.md) | Compare matched workloads with recorded measurement conditions. |
| Prepare a release | [Release procedures](releasing.md) | Complete the maintainer checklist, including installed-wheel and license checks. |

For a larger feature or public API change, discuss the approach in an issue first.

## Extension guides

| Add… | Follow this route |
| --- | --- |
| A source | [Add a source](extending/source.md): model, preparation and backend rendering |
| An effect | [Add a visual effect](extending/effect.md): contract, semantics, passes and kernels |
| A transition | [Add a transition](extending/transition.md): timing, tracks and lowering |
| A backend | [Add a backend](extending/backend.md): lifecycle, capabilities and selection |

<span id="stylization-references"></span>

For palette/dither work, use the [quality checklist](stylization/dithered-palette-look.md)
and [temporal diagnostics](stylization/temporal-diagnostics.md).

<span id="working-with-coding-agents"></span>

Coding agents start with [AGENTS.md](https://github.com/evgen2571/vestra/blob/develop/AGENTS.md).
[Repository skills](https://github.com/evgen2571/vestra/blob/develop/.agents/skills/README.md)
route specialist verification; [PLANS.md](https://github.com/evgen2571/vestra/blob/develop/PLANS.md)
describes substantial-work plans.
