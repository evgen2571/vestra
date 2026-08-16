# Native video media foundation

`vestra-media` owns the v1D1 video foundation. It uses `ffmpeg-next` 7.1.x
behind a Vestra API; the rest of the repository does not handle FFmpeg
contexts or raw frames.

`VideoMediaInfo` is immutable metadata for the first non-attached-picture video
stream that opens with a decoder in stream-index order. Attached-picture
streams and video streams that cannot initialize a decoder are skipped. It preserves the
stream index, coded dimensions, exact time base, stream and container duration,
raw start timestamp, normalized source origin, average frame-rate metadata,
pixel format, sample aspect ratio, and optional `rotate` metadata. Stream
duration is preferred for the source duration; container duration is the
fallback when the selected stream does not provide one.

Source time is normalized from the stream start timestamp: source second `0`
maps to `source_origin`, which is the stream start timestamp when present and
zero otherwise. `seconds_to_timestamp` and `timestamp_to_seconds` are the only
public conversion helpers. Frame identity remains an integer PTS in the
stream's rational time base; frame rate is descriptive and never used for
lookup.

`VideoDecoder` owns one demuxer, decoder, scaler, cursor, pending frame, and
bounded in-memory cache. Create independent decoder instances for independent
workers. `frame_at(T)` returns the latest presentation frame with raw PTS at
or before the normalized target. It seeks and flushes on backward misses, then
decodes forward until the first PTS after the target proves the covering frame.
Nearby monotonic requests reuse the current decode cursor. EOF is drained, and
a later seek flushes the decoder so EOF does not poison the session.

Decoded frames copy into an immutable `Arc<image::RgbaImage>` with RGBA8
straight-alpha pixels. FFmpeg's software scaler handles YUV/RGB conversion;
v1D1 uses its consistent default color conversion and does not introduce a
full color-management policy. Pixel dimensions remain coded dimensions.
Rotation is preserved as metadata and is not applied to pixels. Sample aspect
ratio is preserved as metadata and does not change the returned raster size.
Alpha is preserved when FFmpeg exposes it through the source format and scaler;
it is not broadly validated across alpha-capable codecs yet.

The cache is per decoder, keyed by exact raw PTS, and bounded by a byte budget.
Each RGBA frame is charged as `width * height * 4` with checked arithmetic.
Entries record the next decoded presentation PTS (or the drained stream end),
so an LRU hit is accepted only when it proves the requested covering interval.
LRU eviction therefore remains an optimization only; sparse cache contents can
never change timestamp selection.

Building the native video decoder requires FFmpeg development libraries in
addition to the FFmpeg/FFprobe runtime tools. The supported native ABI is
FFmpeg 7.1.x. Ubuntu 24.04's stock FFmpeg 6.1 packages are insufficient. For
an Ubuntu 24.04 development host, use the same FFmpeg 7 package source as CI,
then install the development packages:

```bash
sudo apt-get install software-properties-common
sudo add-apt-repository ppa:ubuntuhandbook1/ffmpeg7
sudo apt-get update
sudo apt-get install ffmpeg pkg-config \
  libavcodec-dev libavformat-dev libavutil-dev libswscale-dev
```

CI verifies the resulting `libav*` versions with `pkg-config` before building.

The Nix development shell supplies `pkg-config` and `ffmpeg_7-headless`, which
provide the headers, libraries, and pkg-config metadata needed by `ffmpeg-next`.

Frame dimensions, source pixels, RGBA byte counts, and the configured decoded
asset limit are checked before allocation. v1D2 exposes this decoder through the
project-model `Video` source, Python `Video`/`VideoClip`, and both CPU and WGPU
render paths. Rendering uses software FFmpeg decode and uploads RGBA frames to
the WGPU source texture when that backend is selected. Rotation metadata remains
reported but is not applied to pixels; sample aspect ratio likewise remains
metadata-only, so coded raster dimensions are the sizing basis for this phase.
