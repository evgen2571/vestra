# README hero

[Full video](../../../docs/assets/hero/reel.mp4) · [Animation source](main.py)

The 31.8-second reel opens with a Neovim recording of a tile animation and its
Vestra render. The preview zooms into three showcase excerpts, then returns to
the blank editor.

Run [main.py](main.py) from a scratch directory with Vestra installed to render
the two-second tile animation to `out.mp4`. The video pairs that render with the
editor capture and the excerpts below. To regenerate the looping README preview
and the documentation poster from the original MP4, run these commands in this
directory. The poster uses the frame at 15 seconds, when the code and result
are visible together. The WebP keeps the source's 30 FPS and converts the
BT.709 frames to RGB before encoding.

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

The reel uses excerpts from the repository's showcase renders. Each showcase
links its original assets and licenses.

- Compositing uses 0.20–5.80 seconds at 2×, including the landscape photograph
  and Manrope labels. [Compositing asset credits](../compositing/README.md#photo-source)
- Spectrum uses 4.50–10.30 seconds at 1.5×, with the bundled synth driving the
  animation. The reel is silent. [Spectrum asset credits](../audio-visualizer/README.md#credits)
- Video Edit uses 1.30–8.97 seconds at its original speed, including the
  forest-to-valley dissolve. [Video Edit asset credits](../real-media-edit/README.md#shared-inputs)

The Neovim capture uses JetBrains Mono Nerd Font. [Nerd Fonts](https://github.com/ryanoasis/nerd-fonts/tree/master/patched-fonts/JetBrainsMono)
