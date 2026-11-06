# Tutorial media provenance

The files in `renders/` are actual CPU output from the downloadable Python
examples in `examples/`, rendered from this checkout. All graphics are original
Vestra project work under MIT. Text uses Vestra's bundled DejaVu font; the
[font notice](https://github.com/evgen2571/vestra/blob/develop/python/vestra/fonts/LICENSE.txt)
remains with the package. Rendered glyphs do not redistribute the font file.

From the repository root, after [native development setup](../getting-started/installation.md#build-from-source--development):

```bash
just python-sync
just docs-media
```

This runs all four sources with the current checkout's Python engine, writes
full H.264 originals to `target/docs-media/`, probes their dimensions, rate,
decoded frame count and duration, then refreshes the small WebP posters and
two fast-start H.264 previews. The site build uses the committed previews and
never renders a video. Re-encoding bytes may vary with FFmpeg versions.

`home-card.webp` and `first-render.webp` show stationary projects at 1.5 seconds.
`first-animation.webp` shows the settled title at 1.5 seconds. `keyframes.webp`
shows the four easing tracks midway at 1.3 seconds. All projects are 960×540,
30 fps, three seconds, with no audio. Original MP4s are intentionally not committed.

Gallery outputs have separate [provenance](gallery/README.md).
