---
title: Documentation authoring
description: Preview the Vestra documentation site, author readable examples, and verify real rendered output.
---

# Documentation authoring

A reader should leave a page able to make something, understand a decision, or find an exact contract. Choose that purpose before writing. The [first animation](../getting-started/first-animation.md) and [keyframes guide](../getting-started/animation.md) set the standard for new learning content.

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

For media seek checks, see the [preview-server playback limitation](documentation-site.md#preview-server-playback-limitation).

## Choose the page type

| Page type | Reader's question | A useful shape |
| --- | --- | --- |
| Tutorial | “How do I make this?” | Show the result, build it in steps, finish with runnable source. |
| Task guide | “How do I do this in my project?” | State the starting conditions, explain the solution, cover relevant tradeoffs. |
| Concept | “Why does it behave like this?” | Use a concrete case, a visual if helpful, and the rule that explains it. |
| Reference | “What exactly is accepted or returned?” | Give signatures, units, defaults, constraints, errors, and support limits. |

Keep API inventories in reference pages. Link to them when a learner needs more detail. **Learn** contains the introductory sequence, **Guides** organizes tasks by topic, **Concepts** explains the model, and **Reference** gives exact contracts. Put pages in the appropriate navigation section in `zensical.toml`; retain a useful sidebar context on individual gallery pages. When extending a page, reconsider its order and purpose; split or rewrite it if the new material would bury the main task.

### Ownership of recurring topics

| Topic | Maintained home | Other pages |
| --- | --- | --- |
| Installation and first Python render | `getting-started/` | Link here for setup and supported platforms. |
| Effect usage and recipes | `guides/python/effects.md` | The visual catalog shows selected settings checked against its runnable program. |
| Effect contracts | `reference/effects.md` | Concepts explain the processing diagram and link to contracts. |
| Effect defaults, bounds, scope and stage | Generated `reference/effect-parameters.md` | Regenerate with `just gen-effects`; link rather than copying inventories. |
| Groups, local coordinates and time | `guides/python/nested-compositions.md` | `reference/nested-compositions.md` owns exact bounds, limits and canonical form. |
| Benchmark setup | `benchmarks/collecting.md` | The repository benchmark README indexes programs; other pages link to this procedure. |

### Write reference pages for lookup

Start with what the API does and when to use it. Label signatures as inventories,
then group parameters by purpose with defaults, units and restrictions. Separate
construction, placement, snapshot validation and preparation when errors occur
at different stages. Distinguish Python conveniences from canonical JSON and
schema/parser behavior.

The [Text](../reference/sources/text.md) and [Image](../reference/sources/image.md)
references show this structure at different sizes. Use only sections that help
the reader; keep focused examples here and link complete tasks to guides. When
renaming headings, preserve useful incoming anchors and check their generated URLs.

## Use descriptive developer-facing language

Explain what the code does and why a reader would use it. Use headings such as “Position keyframes” or “Rendering a title card” when they describe the section. Avoid slogans, sales language, exaggerated enthusiasm, and dramatic fragments. A friendly tone does not require a tagline or a personality cue in every paragraph.

Choose rendered text, demo titles, labels, captions, and alt text as carefully as prose. A shape study or a named rendering technique gives an example a purpose; a generic inspirational message does not. Review the whole page for tone and repetition rather than substituting a few words. Keep useful context and detail when editing for concision.

## Make examples easy to read and run

Write code someone would be comfortable maintaining. Introduce one operation
per snippet: show a circular crop before combining feathering, animation and
subtraction. Present alternative effects or source types separately rather than
attaching all of them in one demonstration. State whether a block continues the
previous project or starts a new one, and make required media paths visible.

Name the source, layer and attached effect for their roles. Give multi-parameter
effects keyword arguments so readers can identify units and settings. Keep long
source definitions separate from their timeline placement when nesting hides
those two decisions. Use blank lines between setup, content, changes and output;
comments should explain timing, ownership or a choice the code cannot show.
API signatures in reference pages are inventories, labelled as such rather than
presented as runnable Python.

Show imports, setup, rendering, the run command, and the output filename. State when a rerun overwrites an output. Keep optional GPU setup out of a beginner's first render.

Store complete tutorial programs in `examples/tutorials/` and shared inputs in
`assets/`; `docs/assets/` contains only site delivery media and styling. Include
programs directly with `pymdownx.snippets` and link downloads to the maintained
file on `raw.githubusercontent.com/evgen2571/vestra/develop/`:

Put the directive `--8<-- "examples/tutorials/first_animation.py"` on its own line inside a fenced Python block.

The include path is relative to the repository root. The introductory animation is an exact download copy of `examples/showcase/quickstart/main.py`; `just site-check` checks that they match. Do not maintain a separate, subtly different copy of the complete example in Markdown. Small excerpts are useful when they explain one change; check that they still match the full program. Keep the maintained examples formatted and linted:

```bash
uvx --from 'ruff>=0.12,<0.13' ruff check examples/tutorials scripts/render-docs-media.py
uvx --from 'ruff>=0.12,<0.13' ruff format --check examples/tutorials scripts/render-docs-media.py
```

`just site-check` also parses Python snippet syntax and checks the introductory
download against its showcase source. With the native contributor environment,
run `just python-test python-tests/test_documentation_examples.py` to execute the
guide snippets and check that their edits reach the intended layers and effects.
That test captures video render requests; encoding and visual checks remain separate.

Check behavior against the current implementation and its callers/tests. A Python import proves installation, not encoding. Execute the main examples, inspect their actual frames, and record the selected backend when changing a rendering explanation. Document possible API problems separately instead of changing engine behavior to make a tutorial work.

## Let visual concepts be visual

For motion, show the actual rendered result. For timing or coordinate relationships, use a legible diagram when it explains the relationship more clearly than prose. Explain what to notice next to the visual. Avoid repeating the same explanation in the caption and body.

Videos use a small poster, native controls, `playsinline`, and `preload="metadata"` so the controls can show duration before playback. Playback starts when requested; the gallery index uses still previews rather than loading every video. Provide a nearby text description and a download link. Spoken content needs captions. Give informative images meaningful alt text and diagrams a text explanation; do not encode meaning in color alone. Check diagrams at mobile width in both themes. A static picture should remain useful when someone cannot play a video.

## Asset credits

Keep source, author, license, modifications and reproduction instructions beside
the example that uses an asset: in its README for source inputs, and under
**Credits** on its gallery page for site previews. Link to a shared input's record
rather than copying attribution tables. Shared preparation and font provenance
belong in the [showcase README](https://github.com/evgen2571/vestra/blob/bf527c93952b258f518ea6915551437ba629b545/examples/showcase/README.md#prepare-shared-inputs).
The [photo cutouts](https://github.com/evgen2571/vestra/blob/bf527c93952b258f518ea6915551437ba629b545/examples/showcase/compositing/README.md#photo-source)
and [video edit](https://github.com/evgen2571/vestra/blob/bf527c93952b258f518ea6915551437ba629b545/examples/showcase/real-media-edit/README.md#shared-inputs)
show the convention and shared preparation workflow.

Check the actual source terms before making license claims. Credit CC0 inputs
as useful provenance. Preserve attribution for CC BY copies that remain distributed,
including older previews; a replacement asset does not relicense them. Keep full
required license copies and copyright notices beside redistributed font files.
Showing rendered text is distinct from redistributing fonts. Preserve packaged
software notices and the release source/rebuild route when changing credit placement.
`THIRD_PARTY_NOTICES.md` serves software distributions, not as an asset catalog.

Prefer original, deterministic scenes and bundled fonts. Keep originals and QA
frames in `target/`; commit compact previews actually used by the site. Record
poster times and original-render checksums on the relevant gallery page.

## Media and site workflows

<span id="reproduce-tutorial-previews"></span>
<span id="reproduce-visual-learning-studies"></span>
<span id="reproduce-gallery-previews"></span>

Use [Reproduce documentation media](documentation-media.md) for commands,
native dependencies, provenance and inspection criteria:

| Change | Procedure |
| --- | --- |
| Introductory animation | [Tutorial previews](documentation-media.md#reproduce-tutorial-previews) |
| Effect, audio, transition or group studies | [Visual learning studies](documentation-media.md#reproduce-visual-learning-studies) |
| Showcase delivery assets | [Gallery previews](documentation-media.md#reproduce-gallery-previews) |

<span id="motion-tile-anchor-coverage"></span>

For Motion Tile examples, check the
[anchor coverage contract](../reference/effects.md#motion-tile-anchor-coverage).

<span id="keep-the-site-maintainable"></span>
<span id="publish-only-after-review"></span>

For theme overrides, URL behavior and hosting, use [Site maintenance](documentation-site.md).
Its [publication procedure](documentation-site.md#publish-only-after-review) is a
maintainer task after review; previewing or building the site does not deploy it.
