# Tutorial media provenance

The introductory rectangle comes from the
[quickstart source](https://github.com/evgen2571/vestra/blob/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/quickstart/main.py)
on `docs/readme-landing`, revision `a57ddfaecea62035c5042f02d0be397d87262d09`.
The downloadable `examples/first_animation.py` is byte-identical to the imported
`examples/showcase/quickstart/main.py`; the site check enforces that match.
`examples/first_render.py` shows its static starting state, before adding keyframes.
The keyframe comparison reuses the same rounded shape and dark/mint palette.

The files in `renders/` are actual CPU output rendered with the current engine
from the downloadable Python examples. Graphics and code are original MIT-licensed
Vestra project work. The rectangle needs no external assets or fonts. The
comparison labels use the bundled DejaVu font; its
[font notice](https://github.com/evgen2571/vestra/blob/develop/python/vestra/fonts/LICENSE.txt)
remains with the package. Rendered text does not redistribute the font file.

From the repository root, after [native development setup](../getting-started/installation.md#build-from-source--development):

```bash
just python-sync
just docs-media
```

This runs all three sources, writes full H.264 originals to `target/docs-media/`,
probes dimensions, rate, decoded frame count and duration, then refreshes WebP
posters and two fast-start H.264 previews. The website build uses committed
previews and never renders a video. Encoding bytes may vary with FFmpeg versions.

| Web asset | Source output | Dimensions | Poster time |
| --- | --- | --- | --- |
| `first-render.webp` | `first-render.mp4` | 640×360 | 0.5 seconds; static rectangle at x=0.25 |
| `first-animation.webp`, `first-animation.mp4` | `quickstart.mp4` | 640×360 | 0.75 seconds; midway through the first interval |
| `keyframes.webp`, `keyframes.mp4` | `keyframes.mp4` | 960×540 | 1.3 seconds; interpolation comparison |

All projects are 30 fps, three seconds, and silent. Full originals stay in
`target/`; only the delivery copies are committed. Other showcase previews have
separate [provenance](gallery/README.md).
