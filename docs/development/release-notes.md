# v0.1.0 release summary (historical)

Vestra v0.1.0 was published on October 5, 2026. These are historical release
notes, not a pending release checklist. See the [published v0.1.0](https://github.com/evgen2571/vestra/releases/tag/v0.1.0),
[v0.1.1](https://github.com/evgen2571/vestra/releases/tag/v0.1.1), and
[maintainer release procedure](releasing.md).

Vestra's first public release introduces a Python authoring API backed by a
native Rust video composition and rendering engine.

- Build projects with compositions, layers and typed sources; render directly
  or retain prepared resources for frame previews and video output.
- Combine images, video, shapes, text, particles and audio-reactive Spectrum2D
  with keyframes, signals, effects, transitions and nested compositions.
- Use geometric/source masks, alpha/luma track mattes and blend modes.
- Mix audio clips with trimming, fades, automation and audio effects; bind
  graphics to master-audio analysis signals.
- Validate and inspect canonical schema-v1 projects through the Rust/Python
  SDKs or the separately built `ve` CLI. Receive structured diagnostics,
  lifecycle events and render reports.
- Choose CPU or WGPU rendering. Reports identify the selected backend and
  adapter, including software adapters and automatic fallback.

The release wheel matrix covers CPython 3.11, 3.12, 3.13 and 3.14 on Linux
x86_64/aarch64 (glibc 2.28+), macOS Intel/Apple Silicon and Windows x64.
A source distribution is included. Runtime rendering requires `ffmpeg` and
`ffprobe` on `PATH`, including when using a wheel.

The repository includes four complete showcase videos, reproducible Python
projects and licensed assets. Vestra software is MIT licensed; dependencies
and media retain their documented licenses. Corresponding bundled FFmpeg
source archives accompany the GitHub Release downloads.

Backend support has documented limits: effect/transition/source support does
not imply complete visual CPU/WGPU parity. See the
[feature matrix](../reference/feature-support.md). Hardware WGPU availability
and driver behavior depend on the environment. Text requires a font file, and
the Python package does not install the CLI.
