# README hero

[Full video](render.mp4) · [Animation source](main.py)

The 31.8-second reel opens with a Neovim recording of a tile animation and its
Vestra render. The preview zooms into three showcase excerpts, then returns to
the blank editor.

Run [main.py](main.py) from a scratch directory with Vestra installed to render
the two-second tile animation to `out.mp4`. The video pairs that render with the
editor capture and the excerpts below. To regenerate the looping GIF from the
finished MP4, run this command in this directory.

```bash
ffmpeg -i render.mp4 \
  -filter_complex 'fps=10,scale=720:405:flags=lanczos,split[frames][colors];[colors]palettegen=stats_mode=diff:max_colors=160[palette];[frames][palette]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle' \
  -loop 0 preview.gif
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
