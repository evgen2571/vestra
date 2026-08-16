# Native video media foundation

`vestra-media` owns the v1D1 video foundation. It uses `ffmpeg-next` 7.1.x
behind a Vestra API; the rest of the repository does not handle FFmpeg
contexts or raw frames.

`VideoMediaInfo` is immutable metadata for the first usable video stream in
stream-index order. Attached-picture streams are skipped. It preserves the
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
LRU eviction makes the cache an optimization only; cache misses and hits use
the same timestamp-selection rule.

Frame dimensions, source pixels, RGBA byte counts, and the configured decoded
asset limit are checked before allocation. No hardware decode, GPU upload,
audio scheduling, project-model Video Source, Python Video API, or renderer
integration belongs to this phase.
