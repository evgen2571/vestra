# Release notes

## 0.1.5

This release adds more ways to export a project, gives reusable groups their
own canvas, and makes Python installation simpler.

### New

- **Still images and image sequences.** Save a frame as PNG, JPEG or WebP,
  or export selected frames or a timeline range as numbered images.
- **NumPy frame access.** Use `get_frame()` or `Frame.to_numpy()` to get a
  writable RGBA array. Install `vestra[numpy]` to enable it; each array owns
  its pixels independently of the renderer.
- **Partial video exports.** Pass `start` and `end` to render part of a project.
  Effects keep their original timeline position, and audio stays synchronized
  with the selected frames.
- **Previews and output overrides.** Render a quick draft with
  `render_preview()`, or choose a different resolution, frame rate and MP4
  quality without editing the project.
- **GIF and animated WebP.** Export looping, silent animations by choosing
  `.gif` or `.webp`. Both formats stream frames without keeping the whole
  animation in memory; WebP preserves partial transparency.
- **Sized groups.** Give a reusable `Group` its own local canvas with
  `size=(width, height)`. Children use that canvas for positioning and clipping,
  so the same artwork can be placed in projects with different dimensions.
- **Native CLI downloads.** Release artifacts now include separate `ve`
  archives for Linux, macOS and Windows, with checksums and installation notes.
  The CLI runs without Python and still needs FFmpeg on `PATH` for encoding.

### Fixed

- Motion Tile keeps the original source anchor and pattern in place when its
  tiled area expands, including on cropped sources and sized groups.
- Audio probing respects cancellation, and trimmed or delayed audio uses
  sample-based timestamps to preserve mixer timing.
- Sized-group validation checks canvas budgets and addressable sizes before
  rendering. GPU allocation also checks the selected adapter's texture limits.

### Improved

- Python installs its FFmpeg executable through `imageio-ffmpeg`; wheel users
  no longer need a separate FFmpeg installation. Native media probing also
  removes the runtime dependency on `ffprobe`. Use `IMAGEIO_FFMPEG_EXE` to
  select a different encoding executable.
- Documentation now includes playable tutorials, visual effect comparisons,
  clearer explanations of groups and compositing, and reproducible examples.
- Benchmarks use shared footage at 1080p60, with workload previews and a simpler
  collection workflow to make results easier to interpret and reproduce.

Existing groups without `size` keep their previous behavior. Alternate output
formats and render overrides leave the canonical project format unchanged.
See the [rendering guide](docs/guides/python/rendering-and-preparation.md) for
examples and the [release guide](docs/development/releasing.md) for platform
coverage and redistribution requirements.
