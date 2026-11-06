# Gallery preview provenance

These previews are downscaled copies of the repository's Vestra CPU renders.
The original projects render at 1280 × 720, 30 fps. The documentation copies
preserve the full timelines at 640 × 360,
30 fps, using H.264/yuv420p with MP4 fast start. Posters are WebP frames sampled
from those same renders. Only `audio-reactive.mp4` contains sound.

## Sources and licenses

| Documentation files | Original render and complete Python source | Duration | Poster time |
| --- | --- | --- | --- |
| `motion-design.mp4`, `motion-design.webp` | [Animated layers and effects](https://github.com/evgen2571/vestra/tree/develop/examples/showcase/readme-demo) | 10 seconds | 1 second |
| `compositing.mp4`, `compositing.webp` | [Masks and groups](https://github.com/evgen2571/vestra/tree/develop/examples/showcase/compositing) | 8 seconds | 2 seconds |
| `audio-reactive.mp4`, `audio-reactive.webp` | [Audio visualizer](https://github.com/evgen2571/vestra/tree/develop/examples/showcase/audio-visualizer) | 12 seconds | 3 seconds |
| `video-editing.mp4`, `video-editing.webp` | [Video edit](https://github.com/evgen2571/vestra/tree/develop/examples/showcase/real-media-edit) | 14 seconds | 2 seconds |

The source renders were copied from `develop` revision
`38caed6822d7d24aba9ea1de20bf61a8114ccc5c`. Their source scripts and original inputs
remain in `examples/showcase/`. The derivatives were made with FFmpeg 8.1.3;
encoder bytes may vary with FFmpeg versions.

**Footage credit for the animated scenes and video edit:** “City timelapse video”
by Location Kenya, [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:City_timelapse_video.webm).
Trimmed, resized, reframed, colour-adjusted and edited with the original audio
removed. Documentation copies are additionally downscaled and compressed; posters
are extracted frames. Retain this credit and the source/license links when
sharing these previews or derivatives. No author endorsement is implied.

**Photograph in the animated scenes and photo cutouts:** “Landscape in Salzburger
Land” by Dimitry Anikin,
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Landscape_in_Salzburger_Land.jpg).
Resized, cropped, zoomed, masked and colour-adjusted in the rendered scenes.

**Typeface:** Manrope by The Manrope Project Authors,
[SIL Open Font License 1.1](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/assets/Manrope-OFL.txt).
The previews contain rendered text; the font file remains with the source examples.

**Music:** Original synthesized loop by Vestra contributors, dedicated to
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
Original graphic compositions and software are MIT-licensed project work.
Vestra's software license does not replace the third-party terms above.

See the [canonical input asset record](https://github.com/evgen2571/vestra/blob/develop/examples/showcase/ASSETS.md)
for pinned sources, checksums, font notice and original-input reproduction.

## Reproduce the website previews

Run from the repository root with FFmpeg installed. No native engine build is
needed to transcode the committed renders. To regenerate the source renders
instead, follow the [gallery setup](../../gallery/index.md#run-the-examples) and
run the individual Python projects first.

This Bash loop preserves each complete timeline. It replaces the preview files:

```bash
for example in \
  "readme-demo motion-design 1" \
  "compositing compositing 2" \
  "audio-visualizer audio-reactive 3" \
  "real-media-edit video-editing 2"
do
  read -r source name poster_time <<< "$example"
  input="examples/showcase/$source/render.mp4"
  output="docs/assets/gallery/$name"

  audio=(-an)
  if [ "$name" = "audio-reactive" ]; then
    audio=(-c:a aac -b:a 64k)
  fi

  ffmpeg -y -i "$input" -vf scale=640:-2 \
    -c:v libx264 -preset slow -crf 28 -pix_fmt yuv420p \
    "${audio[@]}" -map_metadata -1 -movflags +faststart "$output.mp4"

  ffmpeg -y -ss "$poster_time" -i "$input" -frames:v 1 \
    -vf scale=640:-2 -c:v libwebp -quality 82 \
    -map_metadata -1 "$output.webp"
done
```

The eight preview files total approximately 1.13 MiB. Keep future additions
similarly small: full-resolution originals remain with the examples rather than
being duplicated in the website.
