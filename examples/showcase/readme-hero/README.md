# README hero

[Full video](../../../docs/assets/hero/reel.mp4) · [Animation source](main.py)

The 31.8-second reel shows a tile animation being coded in Neovim, plays its
render, then zooms into three showcase excerpts before returning to the editor.

## Render the tile

With Vestra installed, run [main.py](main.py) from a scratch directory. It
writes the two-second animation to `out.mp4`.

## Regenerate the README preview

Run these commands in this directory with FFmpeg and approve the overwrite
prompts to replace the README WebP
and homepage poster using the original MP4. The WebP keeps the source's 30 fps
and converts BT.709 to RGB. The poster shows the code and render at 15 seconds.

```bash
reel=../../../docs/assets/hero/reel.mp4

ffmpeg -i "$reel" \
  -an -vf 'scale=1152:648:flags=lanczos,format=bgra' \
  -c:v libwebp_anim -quality 82 -compression_level 5 -loop 0 preview.webp

ffmpeg -ss 15.0 -i "$reel" \
  -vf 'scale=960:540:flags=lanczos' -frames:v 1 -c:v libwebp -quality 88 \
  ../../../docs/assets/hero/poster.webp
```

## Credits

| Excerpt | Source range | Playback |
| --- | --- | --- |
| [Compositing](../compositing/README.md#photo-source) | 0.20–5.80 s | 2× |
| [Spectrum](../audio-visualizer/README.md#credits) | 4.50–10.30 s | 1.5×, muted |
| [Video edit](../real-media-edit/README.md#shared-inputs) | 1.30–8.97 s | Original speed |

The Neovim capture uses JetBrains Mono Nerd Font. [Font source](https://github.com/ryanoasis/nerd-fonts/tree/master/patched-fonts/JetBrainsMono)
