# Examples

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
| `effects/` | Individual visual effects, including blur, glow, and camera shake. |
| `transitions/` | Crossfade and directional-push transitions. |
| `presets/` | Timeline presets and flashes. |
| `compositing/` | Blend modes and global post-effects. |
| `python/high-level/11_masks.py` | Layer-owned geometric masks and mask coverage. |
| `projects/` | Complete projects, including animation, audio mixing, and `track-matte.json`. |
| `particles/` | Deterministic ParticleSystem configurations, including audio-reactive appearance. |
| `python/high-level/` | The normal `Project`, `Composition`, `Layer`, and source API, including shape and text rendering. |
| `python/authoring/` | Advanced `ProjectBuilder` examples for canonical authoring and inspection. |

`assets/red.png`, `green.png`, and `blue.png` are shared image fixtures.
`assets/tone.wav` is the shared audio fixture used by audio and signal
examples. `python/high-level/09_video_source.py` intentionally requires a
user-supplied `examples/assets/clip.mp4`; it is not part of the bulk render
run.
