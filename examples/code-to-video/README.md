# Code → Video

A twelve-second, silent 1920×1080 / 30 fps scene: softly muted snowy forest
dissolves into an alpine valley, with a framed Neovim recording inside the composition.
The native placeholder renders today; the later recording uses the same frame
and timing. The project never reads its own output.

From the repository root, with the [development environment](../../CONTRIBUTING.md)
ready and FFmpeg on `PATH`:

```bash
uv run python examples/code-to-video/main.py --backend cpu
# Collection in another directory:
uv run python examples/code-to-video/main.py --assets /path/to/Assets --backend cpu
# Prepare inputs and export/validate the same Project without rendering:
uv run python examples/code-to-video/main.py --export-only
```

All generated files stay in ignored `target/code-to-video/`: `render.mp4`, the
canonical `project.json`, extracted fonts/licenses and silent CFR video inputs
under `prepared/`, `assets.json` with source/prepared SHA-256 fingerprints and
preparation settings, and `render.json` with the selected backend, encoder,
output fingerprint and decoded frame count. Successful output is checked for
1920×1080, 30 fps and 360 decoded frames. Preparation reuses inputs only when
the source/settings and prepared fingerprints match. `--backend wgpu` requests
the GPU renderer; inspect the selected backend in the report.

The default collection is `~/drive/Assets`. Required originals, always read-only:

- `forward-fly-above-snow-forest.mp4` and
  `forward-fly-through-road-to-snow-valley.mp4`: first 6.5 seconds, audio removed,
  center crop to 1920×1080 / 30 fps, H.264 CRF 18 / fast.
- `Cinzel.zip`: `static/Cinzel-Regular.ttf` and `OFL.txt`.
- `Lobster_Two.zip`: `LobsterTwo-Regular.ttf` and `OFL.txt`.

No assets are downloaded or bundled. The font archives carry SIL OFL licenses;
the extracted notices accompany the fonts. Collection footage provenance and
redistribution permission remain with the collection owner; this example does
not assert a license for that footage. The other collection assets and music
are unused.

## Record the code later

Use a **1600×900, 30 fps, 260-second** window recording: four minutes to type,
then twenty seconds holding the completed code. Show only Neovim with
a dark background, a readable monospace font at **22 px** (line height at most
24 px), no sidebar, minimap or wrapping. The final inset is **1200×675**; keep
code within 100 columns. Use an empty `main.py` beside `support.py` in this
example directory. Type **exactly the contents of [main.py](main.py)**, including
the imports, `build_project` function and final `run(build_project)` entry point.
The first docstring is optional. All composition code is in that short file;
`support.py` is already present and does not need to be typed.

Suggested source timing:

| Recording time | Action | Final video time |
| --- | --- | --- |
| 0–240 s | Type the imports, scene and entry point. | 2–9.38 s |
| 240–260 s | Save; hold the completed composition onscreen. | 9.38–10 s |

The **whole take** is retimed into the inset at **2–10 seconds**: the suggested
260-second take plays at **32.5×**. Any valid video of at least eight seconds
works, so take longer to type if needed. The inset fades in during 2–2.5 s and
out during 9.5–10 s. The footage plays through the opening and closing shots;
no recursive second render is needed. Shorter takes and unreadable media fail
before preparation. Other input dimensions are letterboxed to 16:9.

```bash
uv run python examples/code-to-video/main.py --recording /path/to/neovim.mp4 --backend cpu
```

This replaces the placeholder while preserving the composition and exports.
The original recording stays untouched; the prepared eight-second copy is
silent H.264 at 1600×900 / 30 fps. Actual recording integration and its final
legibility review are deferred until a take exists.
