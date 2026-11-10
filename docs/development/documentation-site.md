# Maintain the documentation site

Use this page for theme, URL, browser-verification and hosting changes. To edit
content or preview it locally, start with [Documentation authoring](documentation.md).
To rebuild rendered assets, use [Reproduce documentation media](documentation-media.md).

The site builds from committed assets without the native engine. Use the
[verification procedure](#verify-the-built-site) before submitting a change;
[publication](#publish-only-after-review) is a maintainer task after review.

## Keep the site maintainable

### Configuration and theme

| File | Owns |
| --- | --- |
| `zensical.toml` | Navigation, search, syntax highlighting and theme settings |
| `docs/assets/stylesheets/site.css` | Layout, colors, focus styles and reduced-motion support |
| `overrides/partials/source.html` | Repository link without GitHub statistics requests |

Prefer Zensical's native components, semantic HTML and Markdown. Keep the neutral
light/dark surfaces, visible keyboard focus and reduced-motion behavior. System
fonts avoid remote font requests and work offline after site assets load.

### Identity and keyboard navigation

The identity is text only. Preserve these overrides when upgrading Zensical,
and recheck browser requests and keyboard navigation:

- `overrides/partials/logo.html` suppresses the theme logo; CSS removes its
  empty header/drawer links.
- `overrides/main.html` replaces the theme's `site_meta` block, retaining
  description, canonical and previous/next metadata with an empty `data:,`
  browser icon. Deleting logo/favicon configuration restores theme defaults.
- The mobile sidebar uses `:focus-within` to reveal keyboard-focused links.
  Zensical 0.1.0 otherwise leaves focus on offscreen drawer links.
- Keep `navigation.sections` disabled with Zensical 0.1.0: it makes section
  labels unfocusable even where the mobile view uses them as collapsible controls.

### Links and generated paths

Use relative Markdown file links for other documentation pages. Files outside
`docs/` should link to their explicit GitHub source URL. Pin an imported example
to its source commit when its version differs from `develop`, and record that
provenance beside the example. Never hardcode a domain or `/assets/` in an internal link.

Zensical 0.1.0 rebases raw HTML `src` and `href` paths from the Markdown source
directory, like Markdown links, but leaves `poster` paths unchanged:

| On `getting-started/first-animation.md` | Path |
| --- | --- |
| Video `src` | `../assets/renders/first-animation.mp4` |
| Video `poster` | `../../assets/renders/first-animation.webp` |
| Generated page URL | `getting-started/first-animation/` |

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

The [Documentation workflow](https://github.com/evgen2571/vestra/blob/develop/.github/workflows/docs.yml)
builds and validates matching pull requests and pushes to `develop`. Deployment
requires a manual run on `develop` with **deploy** enabled.

After review, a maintainer must:

1. Ensure the workflow exists on the repository's default branch (currently
   `main`), which GitHub requires to offer `workflow_dispatch`.
2. Set the repository's Pages source to **GitHub Actions** and allow the
   `github-pages` environment to deploy from `develop`.
3. Run **Documentation** on `develop` with **deploy** enabled, then inspect the
   published site at `https://evgen2571.github.io/vestra/`.

No personal access token or `gh-pages` branch is needed. A repository-path change
requires updating `site_url` and rerunning link and browser checks. Local checks
verify the built site; report deployment only after a successful authorized
GitHub run.
