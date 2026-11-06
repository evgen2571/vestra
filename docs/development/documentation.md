---
title: Documentation authoring
description: Preview the Vestra documentation site, author readable examples, and verify real rendered output.
---

# Documentation authoring

A reader should leave a page able to make something, understand a decision, or find an exact contract. Choose that purpose before writing. The [first animation](../learn/first-animation.md) and [keyframes guide](../learn/animation.md) set the standard for new learning content.

## Preview without building the engine

Install [uv](https://docs.astral.sh/uv/getting-started/installation/), then run these commands from the repository root:

```bash
just docs-serve
```

Open the local URL printed by Zensical. Markdown and stylesheet edits reload automatically. Python 3.11 or newer, the pinned Zensical dependencies, and the site sources are sufficient; neither FFmpeg nor a Rust build is needed. Just is a convenience; the direct equivalent is:

```bash
uv run --no-project --with-requirements docs/requirements.txt zensical serve
```

To use an existing Python virtual environment instead of uv, install `docs/requirements.txt` with pip, then run `zensical serve`. Keep that environment separate from the native engine environment.

Before committing:

```bash
just docs-check   # Source Markdown file links across the repository
just site-check   # Clean strict build, generated links/assets/anchors, checker tests
```

`just docs-build` builds only the site into `site/`. The output is disposable and untracked. The generated-site check also verifies the `/vestra/` repository path. It does not check external URLs or judge the technical accuracy of prose.

## Choose the page type

| Page type | Reader's question | A useful shape |
| --- | --- | --- |
| Tutorial | “How do I make this?” | Show the result, build it in steps, finish with runnable source. |
| Task guide | “How do I do this in my project?” | State the starting conditions, explain the solution, cover relevant tradeoffs. |
| Concept | “Why does it behave like this?” | Use a concrete case, a visual if helpful, and the rule that explains it. |
| Reference | “What exactly is accepted or returned?” | Give signatures, units, defaults, constraints, errors, and support limits. |

Keep API inventories in reference pages. Link to them when a learner needs more detail. The [reference hub](../reference/index.md) keeps the existing technical material accessible while the new learning path grows. When extending a page, reconsider its order and purpose; split or rewrite it if the new material would bury the main task.

## Use descriptive developer-facing language

Explain what the code does and why a reader would use it. Use headings such as “Position keyframes” or “Rendering a title card” when they describe the section. Avoid slogans, sales language, exaggerated enthusiasm, and dramatic fragments. A friendly tone does not require a tagline or a personality cue in every paragraph.

Choose rendered text, demo titles, labels, captions, and alt text as carefully as prose. A shape study or a named rendering technique gives an example a purpose; a generic inspirational message does not. Review the whole page for tone and repetition rather than substituting a few words. Keep useful context and detail when editing for concision.

## Make examples easy to read and run

Write code someone would be comfortable maintaining. Use names such as `headline` and `caption`, blank lines between meaningful steps, and comments for decisions the code cannot explain. Show imports, setup, rendering, the run command, and the output filename. State when a rerun overwrites an output. Keep optional GPU setup out of a beginner's first render.

Store complete tutorial programs in `docs/assets/examples/`. Include them directly in fenced code blocks with `pymdownx.snippets`, and offer the same file as a download:

Put the directive `--8<-- "docs/assets/examples/first_animation.py"` on its own line inside a fenced Python block.

The include path is relative to the repository root. The introductory animation is an exact download copy of `examples/showcase/quickstart/main.py`; `just site-check` checks that they match. Do not maintain a separate, subtly different copy of the complete example in Markdown. Small excerpts are useful when they explain one change; check that they still match the full program. Keep the maintained examples formatted and linted:

```bash
uvx --from 'ruff>=0.12,<0.13' ruff check docs/assets/examples scripts/render-docs-media.py
uvx --from 'ruff>=0.12,<0.13' ruff format --check docs/assets/examples scripts/render-docs-media.py
```

Check behavior against the current implementation and its callers/tests. A Python import proves installation, not encoding. Execute the main examples, inspect their actual frames, and record the selected backend when changing a rendering explanation. Document possible API problems separately instead of changing engine behavior to make a tutorial work.

## Let visual concepts be visual

For motion, show the actual rendered result. For timing or coordinate relationships, use a legible diagram when it explains the relationship more clearly than prose. Explain what to notice next to the visual. Avoid repeating the same explanation in the caption and body.

Videos use a small poster, native controls, `playsinline`, and `preload="metadata"` so the controls can show duration before playback. Playback starts when requested; the gallery index uses still previews rather than loading every video. Provide a nearby text description and a download link. Spoken content needs captions. Give informative images meaningful alt text and diagrams a text explanation; do not encode meaning in color alone. Check diagrams at mobile width in both themes. A static picture should remain useful when someone cannot play a video.

For new assets, record the source, license, modifications, and regeneration command. Prefer original, deterministic scenes and bundled fonts. Keep originals and QA frames in `target/`; commit only compact previews used by the site. Follow the [tutorial media provenance](../assets/renders-README.md) and [gallery provenance](../assets/gallery/README.md).

To regenerate the introductory media with the current source checkout:

```bash
# Requires the native contributor environment and FFmpeg, unlike site preview.
just python-sync
just docs-media
```

The renderer runs all three downloadable examples on CPU, probes the original MP4s, and refreshes the web assets. Inspect the new output before committing it. Existing showcase previews can be rebuilt with the commands in the gallery provenance file. Encoding bytes may differ with FFmpeg versions.

## Keep the site maintainable

`zensical.toml` owns navigation, search, syntax highlighting, and theme settings. `docs/assets/stylesheets/site.css` adds Vestra's layout and color choices. The small `overrides/partials/source.html` template keeps the repository link while avoiding GitHub statistics requests; no custom JavaScript is needed. Prefer Zensical's native components, semantic HTML, and Markdown over custom scripts. The palette uses cool paper, ink blue, and coral; the renders carry the visual personality. System fonts avoid a remote font request and keep the site usable offline after loading its assets.

Use relative Markdown file links for other documentation pages. Zensical 0.1.0 rebases raw HTML `src` and `href` paths from the Markdown source directory, just like Markdown links. It leaves `poster` paths unchanged. On `learn/first-animation.md`, use `src="../assets/renders/first-animation.mp4"` but `poster="../../assets/renders/first-animation.webp"` for the generated `learn/first-animation/` URL. The generated-site checker covers both paths; recheck this behavior when upgrading Zensical. Never hardcode a domain or `/assets/` in an internal link. Files outside `docs/` should link to their explicit GitHub source URL. Pin an imported example to its source commit when the version differs from `develop`, and record that provenance alongside its assets. `README.md` and `index.md` map to the same output location, so do not place both in one directory. Check older incoming heading anchors when renaming a section.

Preview desktop and mobile widths. Check navigation, code overflow, light and dark themes, keyboard focus, media controls, and search. Keep body text readable; shorten a passage by removing repetition, not by removing necessary reasoning. There are no diagram or word-count quotas.

## Publish only after review

The Documentation workflow builds and validates changes on pull requests and pushes to `develop`. It deploys only when a maintainer manually runs it on `develop` with **deploy** enabled. Merely merging this configuration does not publish a site.

GitHub only offers `workflow_dispatch` once this workflow exists on the default branch (currently `main`). A maintainer must arrange that activation through the normal reviewed repository workflow; this branch does not change it.

After approval, a maintainer must configure the repository's Pages source as **GitHub Actions** and allow the `github-pages` environment to deploy from `develop`. Then run the Documentation workflow on `develop` with deploy enabled. No personal access token or `gh-pages` branch is needed. The configured URL is `https://evgen2571.github.io/vestra/`; changing the repository path requires updating `site_url` and rerunning link and browser checks.

The workflow configuration can be reviewed locally; a successful Pages deployment requires an authorized run on GitHub. Preserve that distinction in completion reports.
