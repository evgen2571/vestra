# Stylized effects

Try ASCII, dithering, halftone and other looks on the same looping scene.
The script generates its own footage and synth track.

## Try a look

From a [development checkout](../../../CONTRIBUTING.md#2-set-up-a-checkout)
with FFmpeg installed, run either command from the repository root.

```bash
uv run python examples/showcase/stylized-effects/main.py --smoke --look ascii
uv run python examples/showcase/stylized-effects/main.py --smoke --look dither-blue-noise --palette ember
```

Open the MP4s under `target/stylized-showcase/smoke/`. Drop `--smoke` for a
1080p render, or add `--help` to see all looks and controls.

| Try | Change |
| --- | --- |
| Regular and irregular dither patterns | Switch between `--look dither` (Bayer) and `--look dither-blue-noise`. |
| A different palette | Try `--palette mono`, `ember` or `ocean`. |
| A stack of effects | Try `--look analog-monitor`, `halftone-print` or `sorted-neon`. |

[Python source](main.py) · [Effect reference](../../../docs/reference/effects.md)

## Fine-detail reference and ASCII diagnosis

[reference.py](reference.py) renders a generated portrait to compare fine
dither detail and source-colored ASCII.

```bash
uv run python examples/showcase/stylized-effects/reference.py
```

Open the PNGs at their original size under `target/stylized-reference/`.
The same directory holds MP4s, canonical projects and `report.json`. Add
`--full-hd` for 1080p or `--backend wgpu` for GPU output. [Dither study](../../../docs/development/stylization/dithered-palette-look.md) · [Temporal diagnostics](../../../docs/development/stylization/temporal-diagnostics.md)

## Credits

The generated scenes, soundtrack, blue-noise tile and code are original
MIT-licensed work. Custom ASCII uses the test font. Default ASCII uses
the embedded font. [Test font notice](../../../tests/assets/VestraTest-Regular.ttf.license) · [Embedded font notice](../../../crates/vestra-render-common/assets/DejaVuSans.LICENSE)
