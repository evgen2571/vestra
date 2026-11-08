# Reproduce documentation media

Use this page when a documentation change alters a rendered scene or its delivery
assets. Ordinary prose, navigation and theme changes need only the
[site preview and checks](documentation.md#preview-without-building-the-engine).

| Media to rebuild | Route | Prerequisites and output |
| --- | --- | --- |
| Introductory tutorial previews | [Tutorial previews](#reproduce-tutorial-previews) | Native contributor environment, `ffmpeg` and `ffprobe`; CPU renders and web assets. |
| Effect, audio, transition and group studies | [Visual learning studies](#reproduce-visual-learning-studies) | Native environment, `ffmpeg` and `ffprobe`; stills, videos, posters and manifest. |
| Gallery delivery copies | [Gallery previews](#reproduce-gallery-previews) | FFmpeg and existing bundled renders; compact MP4/WebP copies. |

These commands replace generated delivery assets. Inspect actual output before
committing, retain provenance and [asset credits](documentation.md#asset-credits),
and keep diagnostic originals under ignored `target/`.

Preview regeneration uses `ffprobe` as an independent output verifier. This
developer check is separate from Vestra's native media probing, which does not
require the executable.

## Reproduce tutorial previews

To regenerate the introductory media with the current source checkout:

```bash
# Requires the native environment, ffmpeg and ffprobe, unlike site preview.
just python-sync
just docs-media
```

The renderer runs all three downloadable examples on CPU, probes the original MP4s, and refreshes the web assets. Inspect the new output before committing it. Gallery previews can be rebuilt from the checkout as described below. Encoding bytes may differ with FFmpeg versions.

## Reproduce visual learning studies

Learner pages link runnable examples and explain what to notice. Keep encoding,
poster choices, checksums and regeneration records in this page and
[learning-media.json](../assets/renders/learning-media.json).

From the native contributor environment:

```bash
just python-sync
just docs-media --learning
```

| Program in `docs/assets/examples/` | Media | Size and duration | Poster |
| --- | --- | --- | --- |
| `effects_catalog.py` | Source/treatment stills; four synchronized temporal comparisons | 640×520 stills and motion; 3 s for motion | 0.7 s |
| `audio_response.py` | Bounded response, smoothing curves, linear/radial spectra | 640×560, 4 s | 1.2 s |
| `transition_comparisons.py` | Crossfade, push, zoom, Gaussian blur | 640×360, 3 s | 1.5 s |
| `group_placements.py` | One source placed twice, with independent transforms and opacity | 640×360 still | 0.7 s |
| `concepts_masks.py` | Alpha/luma, union/subtract coverage | 640×480, 3 s static | 0.5 s |

Audio studies reserve a lower band for native controls at mobile widths.
Effect comparisons use the same 640×520 framing for stills and videos, with
no encoded control footer or divider; native controls overlay the lower edge
while visible. Video and image cards share the responsive two-column grid.

All learning previews are CPU renders with **no audio streams**. The audio
program still generates a 48 kHz mono PCM16 phrase for analysis: five recurring
bass notes with ascending melody notes. Waveform highlights and playheads expose
timing without sound. Curve samples come from actual prepared signal-bound
markers; the animated markers use the same bindings. Regression checks compare
frames with output audio enabled/disabled and check the plot against those values.

The catalog uses a cropped copy of the existing CC0 landscape, original bright
lettering and a moving striped object. [learning-inputs.txt](../assets/examples/learning-inputs.txt)
records attribution and the exact crop command. Keep this record beside the
photograph and downloadable code. Bundled-font notices remain with Vestra;
no font files are redistributed by these previews.

The renderer stores original MP4s under ignored `target/docs-media/`, exports
compact H.264/yuv420p delivery copies with fast-start metadata and WebP posters,
and probes dimensions, decoded frame counts, timestamps and absence of audio.
The manifest records source-program/input hashes, original video checksums,
backend, dimensions, duration and poster settings. Encoding bytes may vary with
FFmpeg versions; these previews were encoded with FFmpeg 8.1.3. The original introductory previews retain their source settings
in the script's `EXAMPLES` table and regenerate with `just docs-media`.

Inspect beginning/middle/end frames and each comparison at its displayed size.
Compare matching times and parameters; do not accept a subtle or empty result
merely because it encoded. Check manual light/dark switching and mobile diagram
labels as well as file validity. Keep diagnostic frames in `target/` and commit
only delivery assets used by pages.

```bash
uv run pytest python-tests/test_documentation_examples.py python-tests/test_visual_learning_effects.py python-tests/test_documentation_audio_transitions.py
uv run pytest tests/test_docs_media.py
uvx --from 'ruff>=0.12,<0.13' ruff check docs/assets/examples scripts/render-docs-media.py
uvx --from 'ruff>=0.12,<0.13' ruff format --check docs/assets/examples scripts/render-docs-media.py
```

## Reproduce gallery previews

The site uses committed previews, so building it needs no engine, render or network
asset download. From the repository root, with FFmpeg installed, this Bash loop
transcodes the original renders already bundled beside the cleaned sources:

```bash
for example in \
  "readme-demo motion-design 1.7" \
  "compositing compositing 2" \
  "audio-visualizer audio-reactive 3" \
  "real-media-edit video-editing 6" \
  "lookbook visual-effects 2.5"
do
  read -r source name poster_time <<< "$example"
  input="examples/showcase/$source/render.mp4"
  output="docs/assets/gallery/$name"
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

This replaces the delivery copies. The existing previews were encoded with FFmpeg
8.1.3; encoding bytes can vary between versions. Their original-render checksums
are recorded beside the examples. To change the scenes, render their Python
sources first, inspect the output and update the source/render/preview revision
together. Naming, formatting and comment-only source cleanup does not require
re-encoding unchanged scenes. No separate documentation branch is needed.
