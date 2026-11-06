# Examples

The [showcases](showcase/README.md) contain six finished videos with Python
source and bundled licensed assets. Watch the MP4s directly on GitHub, or render
them with `just showcase`.

Start with the [animated README quickstart](showcase/quickstart/README.md).

The [README JSON composition](projects/readme-animation.json) is a complete,
asset-free animated shape for CLI validation and rendering.

The examples below are focused references and deterministic test fixtures.

The JSON projects are the canonical CLI examples. Validate one from the
repository root with:

```bash
target/debug/ve validate examples/projects/animation-effects.json
```

`scripts/render-examples.sh --validate-only` validates every JSON example.
Without flags it also renders one small CPU preview from each category into a
temporary directory. Use `--all` to render all of them, or `--category
particles` to render one category. Pass `--output-dir DIR` to keep outputs.

| Directory | What it demonstrates |
| --- | --- |
| `effects/` | Individual visual effects, including ASCII/pseudo-ASCII, palette/dither, halftone, both pixel-sort directions, CRT, blur, glow, and camera shake. |
| `transitions/` | Crossfade and directional-push transitions. |
| `presets/` | Timeline presets and flashes. |
| `compositing/` | Blend modes and global post-effects. |
| `python/high-level/11_masks.py` | Layer-owned geometric masks and mask coverage. |
| `python/high-level/12_effect_expansion_stress.py` | Motion Tile, directional/radial blur, RGB split, transforms, masks, mattes, and a nested group. |
| `python/high-level/13_palette_dither.py` | Asset-free fine Bayer dithering, three custom palettes, and a looping rainbow phase. |
| [Stylized effects showcase](showcase/stylized-effects/README.md) | Reproducible periodic moving footage, custom glyphs/fonts, all effect families, and a repeating synthesized soundtrack. |
| `projects/` | Complete projects, including animation, audio mixing, and `track-matte.json`. |
| `particles/` | Deterministic ParticleSystem configurations, including audio-reactive appearance. |
| `python/high-level/` | The normal `Project`, `Composition`, `Layer`, and source API, including shape and text rendering. |
| `python/authoring/` | Advanced `ProjectBuilder` examples for canonical authoring and inspection. |

`assets/red.png`, `green.png`, and `blue.png` are shared image fixtures.
`assets/tone.wav` is the shared audio fixture used by audio and signal
examples. `python/high-level/09_video_source.py` intentionally requires a
user-supplied `examples/assets/clip.mp4`; it is not part of the bulk render
run.

[Code → Video](code-to-video/README.md) is a short Python scene with a native
recording placeholder and a matching canonical JSON export. It uses a configurable
local asset collection and keeps all generated media under `target/`.

## Run the Python examples

From the repository root, after the native [contributor setup](../CONTRIBUTING.md#2-set-up-a-checkout):

```bash
just python-sync
uv run python examples/python/high-level/01_quickstart.py --output examples/output/quickstart.mp4
```

The small image and audio fixtures are bundled. Each Python file starts with
its purpose and run command. Start in `python/high-level/`; use `python/authoring/`
when you need explicit asset IDs and canonical model construction.

Output depends on the example:

- The high-level quickstart accepts `--output PATH`. Without it, the encoded
  video is checked in a temporary directory and removed when the script exits.
- `high-level/13_palette_dither.py` keeps its MP4 under `examples/output/` and
  preserves the matrix, palette-matching and seed options shown by `--help`.
- Other high-level examples author a scene or prepare one CPU frame and print
  a short result. They do not save a video; the video-source example also needs
  your own `examples/assets/clip.mp4` with at least six seconds of footage.
- The advanced authoring examples demonstrate building, inspecting or preparing
  canonical projects. `07_music_mix.py`, `08_audio_reactive.py` and
  `09_spectrum2d.py` encode temporary videos to check their output, then remove
  them. A builder's `output_path` setting alone does not render a file.

For complete videos you can keep and modify, use the [showcases](showcase/README.md).
Their individual READMEs give input requirements, run commands, output paths
and credits. Diagnostic tools such as the stylized-effects reference also keep
their generated source, frames and reports under `target/`.
