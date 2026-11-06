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

The pinned Zensical 0.1.0 preview server does not answer HTTP byte-range
requests. Chromium can therefore reset a seek in a silent MP4 to its beginning,
even when the same file plays and seeks correctly on a range-capable server.
For seek verification, serve the built `site/` directory with byte-range support
(or download the MP4 and inspect it locally). Keep the native controls; this is
a preview-server limitation, not a reason to replace the media player. The
repository-path check still needs the site mounted at `/vestra/`.

## Choose the page type

| Page type | Reader's question | A useful shape |
| --- | --- | --- |
| Tutorial | “How do I make this?” | Show the result, build it in steps, finish with runnable source. |
| Task guide | “How do I do this in my project?” | State the starting conditions, explain the solution, cover relevant tradeoffs. |
| Concept | “Why does it behave like this?” | Use a concrete case, a visual if helpful, and the rule that explains it. |
| Reference | “What exactly is accepted or returned?” | Give signatures, units, defaults, constraints, errors, and support limits. |

Keep API inventories in reference pages. Link to them when a learner needs more detail. **Learn** contains the introductory sequence, **Guides** organizes tasks by topic, **Concepts** explains the model, and **Reference** gives exact contracts. Put pages in the appropriate navigation section in `zensical.toml`; retain a useful sidebar context on individual gallery pages. When extending a page, reconsider its order and purpose; split or rewrite it if the new material would bury the main task.

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

Store complete tutorial programs in `docs/assets/examples/`. Include them directly in fenced code blocks with `pymdownx.snippets`, and offer the same file as a download:

Put the directive `--8<-- "docs/assets/examples/first_animation.py"` on its own line inside a fenced Python block.

The include path is relative to the repository root. The introductory animation is an exact download copy of `examples/showcase/quickstart/main.py`; `just site-check` checks that they match. Do not maintain a separate, subtly different copy of the complete example in Markdown. Small excerpts are useful when they explain one change; check that they still match the full program. Keep the maintained examples formatted and linted:

```bash
uvx --from 'ruff>=0.12,<0.13' ruff check docs/assets/examples scripts/render-docs-media.py
uvx --from 'ruff>=0.12,<0.13' ruff format --check docs/assets/examples scripts/render-docs-media.py
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
belong in the [showcase README](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/README.md#prepare-shared-inputs).
The [photo cutouts](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/compositing/README.md#photo-source)
and [video edit](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/examples/showcase/real-media-edit/README.md#footage-source)
show the convention, including original checksums and processing steps.

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

## Reproduce tutorial previews

To regenerate the introductory media with the current source checkout:

```bash
# Requires the native contributor environment and FFmpeg, unlike site preview.
just python-sync
just docs-media
```

The renderer runs all three downloadable examples on CPU, probes the original MP4s, and refreshes the web assets. Inspect the new output before committing it. Gallery previews can be rebuilt from the checkout as described below. Encoding bytes may differ with FFmpeg versions.

## Reproduce visual learning studies

The concepts, transition, effect-catalog and audio studies use original
procedural artwork, generated audio and bundled fonts. From the native
contributor environment:

```bash
just python-sync
just docs-media --learning
```

The maintained sources are `docs/assets/examples/concepts_masks.py`,
`effects_catalog.py`, `transition_comparisons.py` and `audio_response.py`. Each
runs independently; download links and exact run commands live on its guide.
The shared renderer writes originals to `target/docs-media/`, makes compact
H.264/WebP delivery copies, checks decoded frame counts/timestamps and AAC
presence/duration, and writes
[learning-media.json](../assets/renders/learning-media.json). That manifest records
source-program hashes, original-video checksums, CPU selection and poster times;
encoding bytes may vary with FFmpeg versions. It also records static previews at
0.7 seconds. The mask comparison is 640×480; the other studies are 640×360.

Inspect all new static frames and sample each moving result before accepting a
regeneration. Frame counts alone do not establish a useful comparison or audio
synchronization. Compare matching source/timing/settings, inspect beginning,
middle and end, and listen to the pulse studies alongside the bound motion.
Keep originals/contact sheets in ignored `target/` and commit only delivery assets
used by a page. There is no speech to caption in these original tone studies.

Run the maintained code checks as well as the site checks:

```bash
uv run pytest python-tests/test_documentation_examples.py python-tests/test_visual_learning_effects.py
uv run pytest tests/test_docs_media.py
uvx --from 'ruff>=0.12,<0.13' ruff check docs/assets/examples scripts/render-docs-media.py
uvx --from 'ruff>=0.12,<0.13' ruff format --check docs/assets/examples scripts/render-docs-media.py
```

## Reproduce gallery previews

The site uses committed previews, so building it needs no engine, render or network
asset download. From the repository root, with FFmpeg installed, this Bash loop
transcodes the original renders already bundled beside the cleaned sources:

```bash
for example in \
  "readme-demo motion-design 1.7" \
  "compositing compositing 2" \
  "audio-visualizer audio-reactive 3" \
  "real-media-edit video-editing 6" \
  "lookbook visual-effects 2.5"
do
  read -r source name poster_time <<< "$example"
  input="examples/showcase/$source/render.mp4"
  output="docs/assets/gallery/$name"
  audio=(-an)
  if [ "$name" = "audio-reactive" ]; then
    audio=(-c:a aac -b:a 80k)
  fi
  ffmpeg -y -i "$input" -vf scale=640:-2:flags=lanczos \
    -c:v libx264 -preset slow -crf 27 -pix_fmt yuv420p \
    "${audio[@]}" -map_metadata -1 -movflags +faststart "$output.mp4"
  ffmpeg -y -ss "$poster_time" -i "$input" -frames:v 1 \
    -vf scale=640:-2:flags=lanczos -c:v libwebp -quality 86 \
    -map_metadata -1 "$output.webp"
done
```

This replaces the delivery copies. The existing previews were encoded with FFmpeg
8.1.3; encoding bytes can vary between versions. Their original-render checksums
are recorded beside the examples. To change the scenes, render their Python
sources first, inspect the output and update the source/render/preview revision
together. Naming, formatting and comment-only source cleanup does not require
re-encoding unchanged scenes. No separate documentation branch is needed.

## Keep the site maintainable

`zensical.toml` owns navigation, search, syntax highlighting, and theme settings. `docs/assets/stylesheets/site.css` adds Vestra's layout and color choices. The `overrides/partials/source.html` template keeps the repository link while avoiding GitHub statistics requests; no custom JavaScript is needed. Prefer Zensical's native components, semantic HTML, and Markdown over custom scripts. The palette uses neutral grays: soft paper in light mode and charcoal in dark mode; the renders carry the visual personality. Links and controls use a contrasting hover surface and short movement, with keyboard focus and reduced-motion support. System fonts avoid a remote font request and keep the site usable offline after loading its assets.

The identity is text only. `overrides/partials/logo.html` suppresses the theme
logo, and CSS removes its empty header/drawer links. `overrides/main.html`
overrides the pinned theme's `site_meta` block to retain description, canonical
and previous/next metadata while setting an empty `data:,` browser icon. Merely
deleting logo/favicon configuration would restore theme defaults. Recheck these
overrides and browser requests when upgrading Zensical. The mobile sidebar also
uses `:focus-within` to reveal links when reached by keyboard; Zensical 0.1.0
otherwise leaves focus on offscreen drawer links. Removing the asset from
this branch does not purge earlier commits, caches, forks or clones.

Use relative Markdown file links for other documentation pages. Zensical 0.1.0 rebases raw HTML `src` and `href` paths from the Markdown source directory, just like Markdown links. It leaves `poster` paths unchanged. On `learn/first-animation.md`, use `src="../assets/renders/first-animation.mp4"` but `poster="../../assets/renders/first-animation.webp"` for the generated `learn/first-animation/` URL. The generated-site checker covers both paths; recheck this behavior when upgrading Zensical. Never hardcode a domain or `/assets/` in an internal link. Files outside `docs/` should link to their explicit GitHub source URL. Pin an imported example to its source commit when the version differs from `develop`, and record that provenance beside the example. `README.md` and `index.md` map to the same output location, so do not place both in one directory. Check older incoming heading anchors when renaming a section.

Preview desktop and mobile widths. Check navigation, code overflow, light and dark themes, keyboard focus, media controls, and search. Keep body text readable; shorten a passage by removing repetition, not by removing necessary reasoning. There are no diagram or word-count quotas.

## Publish only after review

The Documentation workflow builds and validates changes on pull requests and pushes to `develop`. It deploys only when a maintainer manually runs it on `develop` with **deploy** enabled. Merely merging this configuration does not publish a site.

GitHub only offers `workflow_dispatch` once this workflow exists on the default branch (currently `main`). A maintainer must arrange that activation through the normal reviewed repository workflow; this branch does not change it.

After approval, a maintainer must configure the repository's Pages source as **GitHub Actions** and allow the `github-pages` environment to deploy from `develop`. Then run the Documentation workflow on `develop` with deploy enabled. No personal access token or `gh-pages` branch is needed. The configured URL is `https://evgen2571.github.io/vestra/`; changing the repository path requires updating `site_url` and rerunning link and browser checks.

The workflow configuration can be reviewed locally; a successful Pages deployment requires an authorized run on GitHub. Preserve that distinction in completion reports.
