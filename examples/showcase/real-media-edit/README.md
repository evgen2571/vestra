# Video editing

[Watch / download the rendered video](render.mp4)

![Animated preview of the Vestra render](preview.gif)

A fourteen-second video editing example: three day-to-night Shanghai selections, slow reframing,
two dissolves, muted saturation/vignette, a grouped title and attribution. The output is silent.

From the checkout after `uv sync --locked --extra dev`:

```bash
uv run python examples/showcase/real-media-edit/main.py
```

Output: `examples/output/real-media-edit.mp4`, 1280×720, 30 fps, 14 seconds, CPU.
Pass `--smoke` for the complete timeline at 320×180 and 6 fps, or `--output PATH`.
The three five-second selections are bundled, so the checkout
renders without a download. `prepare-assets.py` can reproduce them from the
verified original (about 18 MiB), which stays in an ignored local cache.

## Footage source

`assets/city-1.mp4`, `city-2.mp4` and `city-3.mp4` are selections from
“Shanghai Bund, Lujiazui Skyline Timelapse” by Yigang Yu,
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Shanghai_Bund,_Lujiazui_Skyline_Timelapse.webm).
Commons dedicates it to [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/);
the [creator's Flickr page](https://www.flickr.com/photos/133693764@N05/19938038713/)
marks it as a Public Domain Work. Both statements and the CC0 legal code were
checked on 2026-11-06. Credit is retained as provenance; CC0 does not require it.

Five-second selections begin at 3, 15 and 27 seconds (day, blue hour and night).
Preparation scales 1920×1080 to 1280×720 with Lanczos, converts 29.97 to 30 fps
H.264, and removes audio and metadata. The edit then reframes, grades and dissolves
them. The [README demo](../readme-demo/README.md) uses the blue-hour selection
through a rotating mask and dither effect.

The [preparation script](../prepare-assets.py) pins the original URL and verifies
SHA-256 `6e506bc88f6912a931d29bd02464606a39402b5203726fbdd66facb35ddc698f`.
Run `just showcase-assets` to reproduce the selections; the roughly 18 MiB original
stays under `target/showcase-sources/`. See [shared preparation](../README.md#prepare-shared-inputs).

Labels use Manrope; its [copyright and complete OFL license](../README.md#shared-font)
remain with the font. Videos display rendered text rather than distributing the
font file. Code and original graphics retain Vestra's MIT license.

### Earlier footage copies

Earlier releases, branches, clones or shared previews may still contain “City
timelapse video” by Location Kenya, [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/),
[from Wikimedia Commons](https://commons.wikimedia.org/wiki/File:City_timelapse_video.webm).
Those copies keep their license. When sharing them, retain: “City timelapse video —
Location Kenya, CC BY 3.0; trimmed, resized, reframed, colour-adjusted and edited
with the original audio removed.” Include the source and license links above;
no author endorsement is implied. Replacing this checkout's footage does not
relicense older copies.
