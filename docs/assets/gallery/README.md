# Gallery preview provenance

These previews are downscaled copies of Vestra CPU renders from the
[`docs/readme-landing` source revision](https://github.com/evgen2571/vestra/commit/a57ddfaecea62035c5042f02d0be397d87262d09),
`a57ddfaecea62035c5042f02d0be397d87262d09`. The original projects render at
1280 × 720 and 30 fps using the high quality tier. The documentation copies
preserve the complete timelines at 640 × 360 and 30 fps, using H.264/yuv420p
with MP4 fast start. Posters are WebP frames from those same renders.
Only `audio-reactive.mp4` contains sound.

The gallery's source and full-resolution video links are pinned to this commit
rather than a moving branch. They remain available while the documentation
branch is under review.

## Sources and licenses

| Documentation files | Original render and complete Python source | Duration | Poster time |
| --- | --- | --- | --- |
| `motion-design.mp4`, `motion-design.webp` | [Layers, masks and audio signals](https://github.com/evgen2571/vestra/tree/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/readme-demo) | 10 seconds | 1.7 seconds |
| `compositing.mp4`, `compositing.webp` | [Masks and groups](https://github.com/evgen2571/vestra/tree/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/compositing) | 8 seconds | 2 seconds |
| `audio-reactive.mp4`, `audio-reactive.webp` | [Audio visualizer](https://github.com/evgen2571/vestra/tree/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/audio-visualizer) | 12 seconds | 3 seconds |
| `video-editing.mp4`, `video-editing.webp` | [Video edit](https://github.com/evgen2571/vestra/tree/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/real-media-edit) | 14 seconds | 6 seconds |
| `visual-effects.mp4`, `visual-effects.webp` | [Halftone and ASCII effects](https://github.com/evgen2571/vestra/tree/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/lookbook) | 6 seconds | 2.5 seconds |

**Footage in the animated scenes and video edit:** “Shanghai Bund, Lujiazui
Skyline Timelapse” by Yigang Yu,
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Shanghai_Bund,_Lujiazui_Skyline_Timelapse.webm)
and [the original Flickr page](https://www.flickr.com/photos/133693764@N05/19938038713/).
The three source selections begin at 3, 15 and 27 seconds: daytime, blue hour
and night. They are trimmed, resized, reframed, colour-adjusted and edited with
the original audio removed. The animated scenes also mask and dither the
blue-hour selection. Documentation copies are further downscaled and compressed;
posters are extracted frames. Attribution is retained as provenance, though
CC0 does not require it.

**Photograph in the photo cutouts and effects comparison:** “Landscape in
Salzburger Land” by Dimitry Anikin,
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Landscape_in_Salzburger_Land.jpg).
Resized, cropped, zoomed, masked, colour-adjusted and processed with halftone
and ASCII effects in the respective scenes.

**Typefaces:** Manrope by The Manrope Project Authors,
[SIL Open Font License 1.1](https://github.com/evgen2571/vestra/blob/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/assets/Manrope-OFL.txt).
The ASCII effect uses Vestra's bundled DejaVu face, with its
[Bitstream Vera/DejaVu notice](https://github.com/evgen2571/vestra/blob/a57ddfaecea62035c5042f02d0be397d87262d09/crates/vestra-render-common/assets/DejaVuSans.LICENSE).
The previews contain rendered text; no additional font files are distributed
in the website.

**Music:** Original synthesized loop by Vestra contributors, dedicated to
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
Original graphic compositions and software are MIT-licensed project work.
Vestra's software license does not replace the third-party terms above.

See the [canonical input asset record](https://github.com/evgen2571/vestra/blob/a57ddfaecea62035c5042f02d0be397d87262d09/examples/showcase/ASSETS.md)
for pinned sources, checksums, font notices and input reproduction.

## Reproduce the website previews

Run from the repository root with Git and FFmpeg installed. No native engine
build is needed to transcode the source renders. To render the Python projects
instead, follow the [gallery setup](../../gallery/index.md#run-the-examples) and
run their individual scripts.

This Bash loop reads the captured render files directly from Git and replaces
the website previews. Temporary full-resolution inputs stay under `target/`:

```bash
git fetch origin docs/readme-landing
revision=a57ddfaecea62035c5042f02d0be397d87262d09
mkdir -p target/gallery-preview-sources docs/assets/gallery

for example in \
  "readme-demo motion-design 1.7" \
  "compositing compositing 2" \
  "audio-visualizer audio-reactive 3" \
  "real-media-edit video-editing 6" \
  "lookbook visual-effects 2.5"
do
  read -r source name poster_time <<< "$example"
  input="target/gallery-preview-sources/$name.mp4"
  output="docs/assets/gallery/$name"
  git show "$revision:examples/showcase/$source/render.mp4" > "$input"

  audio=(-an)
  if [ "$name" = "audio-reactive" ]; then
    audio=(-c:a aac -b:a 80k)
  fi

  ffmpeg -y -i "$input" -vf scale=640:-2:flags=lanczos \
    -c:v libx264 -preset slow -crf 27 -pix_fmt yuv420p \
    "${audio[@]}" -map_metadata -1 -movflags +faststart "$output.mp4"

  ffmpeg -y -ss "$poster_time" -i "$input" -frames:v 1 \
    -vf scale=640:-2:flags=lanczos -c:v libwebp -quality 86 \
    -map_metadata -1 "$output.webp"
done
```

The derivatives were made with FFmpeg 8.1.3. Encoder bytes can differ between
versions; the source render checksums below identify the exact inputs.

| Source render | SHA-256 |
| --- | --- |
| `readme-demo/render.mp4` | `38b6d16c31f5580578029ff5f4613a20e10a7883965bcadec9231892bf9e752e` |
| `compositing/render.mp4` | `382c4376de3ebfd5dc68cbe40239b00402da8a05d6db1ee478dc3642e5c2903b` |
| `audio-visualizer/render.mp4` | `2abfd5976d5c948cc77aaec688fd73f6bd9044940ecc8b5718a313f9d97f4db5` |
| `real-media-edit/render.mp4` | `16ac337002626f69e6797f9291ad6b3bea5967f4da32a81451a2a0ace52ff933` |
| `lookbook/render.mp4` | `91455e376d50a5a91d0746dccb1304b0b0b8a57d8db1fb13cdd0bd1ab7f3bcda` |

The ten preview files total approximately 1.22 MiB. Full-resolution originals
remain with the examples rather than being duplicated in the website.
