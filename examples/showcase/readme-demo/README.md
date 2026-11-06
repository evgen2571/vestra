# Video composition showcase

[Watch / download the silent video](render.mp4) · [Python source](main.py)

![Vestra hero: nested cards, masked footage and an audio-reactive spectrum](preview.gif)

The README's ten-second hero is itself a Vestra composition, including all text:

- Rounded cards and their inset labels move together in nested compositions.
- Blue-hour Shanghai footage plays through a rotating mask, then becomes a
  four-colour Bayer-dithered image.
- A radial spectrum and RMS-driven ring respond to the bundled synth recording.

Python and JSON share the top label; scene captions explain the changing visual.
The synth remains in the analysis mix, with `output_audio=False` suppressing the
exported audio track. No audio is added or stripped in a separate encoding step.
All inputs are bundled; no download is needed. See [asset credits](../ASSETS.md)
for the CC0 footage/audio and OFL font.

```bash
uv sync --locked --extra dev
uv run python examples/showcase/readme-demo/main.py
```

Output: `examples/output/readme-demo.mp4`, 1280×720, 30 fps, 10 seconds, CPU, no audio.
Use `--smoke` for the same timeline at 320×180 and 6 fps, or `--output PATH`.
`just showcase-refresh` regenerates committed videos, posters and animated previews.

The root README uses a
[GitHub-hosted video attachment](https://github.com/user-attachments/assets/9e7044ba-c4d5-49fc-a0aa-254eb1649b59)
so it plays inline. When replacing the hero, attach the new render to the existing PR with
`gh pr comment --attach examples/showcase/readme-demo/render.mp4` (gh 2.99+) or
use GitHub’s Markdown editor, then replace the bare attachment URL in the README. Update the checksums below too; changing the repository MP4 alone does not
update the attachment.

<details>
<summary>Published hero checksums (SHA-256)</summary>

```text
38b6d16c31f5580578029ff5f4613a20e10a7883965bcadec9231892bf9e752e  render.mp4
15b4e3cdb3f525ae790b74b79754a29d6d8c87c440a32bd8bbe44b9a4728d53b  main.py
```

</details>
