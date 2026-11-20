# Examples

| Start here | What you'll find |
| --- | --- |
| [Showcases](showcase/README.md) | Finished videos with previews and Python source. |
| [First animation](../docs/getting-started/first-animation.md) | A step-by-step tutorial for animating a shape. |
| [JSON animation](projects/readme-animation.json) | An asset-free project you can render with the CLI. |

## Run a Python example

Run from the checkout after [setting up the development environment](../CONTRIBUTING.md#2-set-up-a-checkout).

```bash
uv run python examples/python/high-level/01_quickstart.py --output examples/output/quickstart.mp4
```

The snippets start with their run instructions. The video-source example needs
your own six-second clip at `assets/fixtures/clip.mp4`. [High-level Python examples](python/high-level/) · [ProjectBuilder examples](python/authoring/) · [Tutorial sources](tutorials/README.md)

## Run a JSON example

With [`ve` installed](../docs/getting-started/cli-installation.md), run from the repository root.

```bash
ve validate examples/projects/readme-animation.json
ve render examples/projects/readme-animation.json --render-backend cpu --output animation.mp4
```

| Browse JSON projects | What they show |
| --- | --- |
| [Complete projects](projects/) | Animation, audio mixing, masks and mattes |
| [Effects](effects/) · [Compositing](compositing/) | Individual effects, blending and post-processing |
| [Transitions](transitions/) · [Presets](presets/) | Dissolves, pushes and reusable motion |
| [Particles](particles/) | Particle systems, including audio-reactive motion |

Use `just examples` to validate the full collection. [CLI quickstart](../docs/getting-started/cli-quickstart.md)
