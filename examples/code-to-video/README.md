# Code → Video

A silent, twelve-second edit with a Neovim window over snowy forest and valley
footage. The script renders a placeholder until you supply a recording. [Scene source](main.py)

## Render

Use a [development checkout](../../CONTRIBUTING.md#2-set-up-a-checkout) with
`ffmpeg` and `ffprobe` on your PATH. Put these files in `~/drive/Assets` or pass
your own directory with `--assets PATH`.

| Input | Required files |
| --- | --- |
| Footage | `forward-fly-above-snow-forest.mp4`, `forward-fly-through-road-to-snow-valley.mp4` |
| Fonts | `Cinzel.zip` (`static/Cinzel-Regular.ttf`) and `Lobster_Two.zip` (`LobsterTwo-Regular.ttf`), each with `OFL.txt` |

```bash
uv run python examples/code-to-video/main.py --backend cpu
```

Open `target/code-to-video/render.mp4`. A rerun replaces it. The same directory
holds prepared inputs and reports. Use `--export-only` to write and validate
`project.json` without rendering.

## Record the code later

Record [main.py](main.py) in Neovim at 1600 × 900 and 30 fps. Use a readable
monospace font around 22 px and keep the completed code on screen briefly at
the end. The take must be at least eight seconds. The edit speeds up the whole
take to fit the inset at 2–10 seconds.

```bash
uv run python examples/code-to-video/main.py --recording /path/to/neovim.mp4 --backend cpu
```

## Asset credits

The script reads your local footage and keeps the font notices with the
prepared copies. Check your footage's source and terms before sharing a render.
