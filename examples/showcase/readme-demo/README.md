# Video composition showcase

[Watch / download the silent video](render.mp4) · [Python source](main.py)

![Vestra hero: animated cards, masked footage and palette dithering](preview.gif)

The README's ten-second hero is itself a Vestra composition, including all text:

- Three rounded cards rotate and move with eased keyframes.
- A night-city clip plays through a rounded mask with animated rotation.
- A photograph transitions into a four-colour Bayer-dithered look.

Scene labels identify the features, and a progress line follows the timeline.
All inputs are bundled; no download is needed. Footage is by Location Kenya
under CC BY 3.0; see [asset credits](../ASSETS.md) for source/license links and
redistribution requirements. This replaces the earlier feature-by-feature reel.

```bash
uv sync --locked --extra dev
uv run python examples/showcase/readme-demo/main.py
```

Output: `examples/output/readme-demo.mp4`, 1280×720, 30 fps, 10 seconds, CPU, no audio.
Use `--smoke` for the same timeline at 320×180 and 6 fps, or `--output PATH`.
`just showcase-refresh` regenerates committed videos, posters and animated previews.

The root README uses a
[GitHub-hosted video attachment](https://github.com/user-attachments/assets/acf3df91-ce40-4b98-91ff-f0fe31963d11)
so it plays inline. When replacing the hero, upload the new render
through GitHub's Markdown editor and replace the bare attachment URL in the
README. Update the checksums below too; changing the repository MP4 alone does not
update the attachment.

<details>
<summary>Published hero checksums (SHA-256)</summary>

```text
42e89d83a7162dc38ac2341b964e369f3d82b04e9b7d851af0767dc9903681d8  render.mp4
0740ff9684c38c5d3fa9a4c355ca61244c9137111c7e958b91d550fa5d1286f9  main.py
```

</details>
