# Maintain the documentation site

Use this page for theme, URL, browser-verification and hosting changes. To edit
content or preview it locally, start with [Documentation authoring](documentation.md).
To rebuild rendered assets, use [Reproduce documentation media](documentation-media.md).

The site builds from committed assets without the native engine. A local build
and browser review establish site behavior; publication requires a separate,
authorized maintainer workflow.

## Keep the site maintainable

### Configuration and theme

`zensical.toml` owns navigation, search, syntax highlighting, and theme settings. `docs/assets/stylesheets/site.css` adds Vestra's layout and color choices. The `overrides/partials/source.html` template keeps the repository link while avoiding GitHub statistics requests; no custom JavaScript is needed. Prefer Zensical's native components, semantic HTML, and Markdown over custom scripts. The palette uses neutral grays: soft paper in light mode and charcoal in dark mode; the renders carry the visual personality. Links and controls use a contrasting hover surface and short movement, with keyboard focus and reduced-motion support. System fonts avoid a remote font request and keep the site usable offline after loading its assets.

### Identity and keyboard navigation

The identity is text only. `overrides/partials/logo.html` suppresses the theme
logo, and CSS removes its empty header/drawer links. `overrides/main.html`
overrides the pinned theme's `site_meta` block to retain description, canonical
and previous/next metadata while setting an empty `data:,` browser icon. Merely
deleting logo/favicon configuration would restore theme defaults. Recheck these
overrides and browser requests when upgrading Zensical. The mobile sidebar also
uses `:focus-within` to reveal links when reached by keyboard; Zensical 0.1.0
otherwise leaves focus on offscreen drawer links. Removing the asset from
this branch does not purge earlier commits, caches, forks or clones.

### Links and generated paths

Use relative Markdown file links for other documentation pages. Files outside
`docs/` should link to their explicit GitHub source URL. Pin an imported example
to its source commit when its version differs from `develop`, and record that
provenance beside the example. Never hardcode a domain or `/assets/` in an internal link.

Zensical 0.1.0 rebases raw HTML `src` and `href` paths from the Markdown source
directory, like Markdown links, but leaves `poster` paths unchanged:

| On `learn/first-animation.md` | Path |
| --- | --- |
| Video `src` | `../assets/renders/first-animation.mp4` |
| Video `poster` | `../../assets/renders/first-animation.webp` |
| Generated page URL | `learn/first-animation/` |

The generated-site checker covers both asset paths; recheck this behavior when
upgrading Zensical. `README.md` and `index.md` map to the same output location, so
do not put both in one directory. Check older incoming anchors when renaming sections.

## Verify the built site

Run `just docs-check` and `just site-check` after changing navigation, templates,
styles or paths. The generated-site check covers internal links, heading anchors,
media references and the `/vestra/` repository prefix. It does not check external
URLs, technical prose accuracy or a live deployment.

Inspect desktop and mobile widths in both themes. Use the keyboard to reach the
menu, search, navigation links, code and expandable details. Check table and code
scrolling, diagram labels, focus visibility and direct links to moved anchors.
The [authoring guide](documentation.md#let-visual-concepts-be-visual) covers media
accessibility.

### Preview-server playback limitation

The pinned Zensical 0.1.0 preview server does not answer HTTP byte-range
requests. Chromium can therefore reset a seek in a silent MP4 to its beginning,
even when the same file plays and seeks correctly on a range-capable server.
For seek verification, serve the built `site/` directory with byte-range support
(or download the MP4 and inspect it locally). Keep the native controls; this is
a preview-server limitation, not a reason to replace the media player. The
repository-path check still needs the site mounted at `/vestra/`.

## Publish only after review

The Documentation workflow builds and validates changes on pull requests and pushes to `develop`. It deploys only when a maintainer manually runs it on `develop` with **deploy** enabled. Merely merging this configuration does not publish a site.

GitHub only offers `workflow_dispatch` once this workflow exists on the default branch (currently `main`). A maintainer must arrange that activation through the normal reviewed repository workflow; this branch does not change it.

After approval, a maintainer must configure the repository's Pages source as **GitHub Actions** and allow the `github-pages` environment to deploy from `develop`. Then run the Documentation workflow on `develop` with deploy enabled. No personal access token or `gh-pages` branch is needed. The configured URL is `https://evgen2571.github.io/vestra/`; changing the repository path requires updating `site_url` and rerunning link and browser checks.

The workflow configuration can be reviewed locally; a successful Pages deployment requires an authorized run on GitHub. Preserve that distinction in completion reports.
