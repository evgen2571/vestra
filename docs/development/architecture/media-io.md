# Media I/O

`vestra-media` owns every transition between Vestra values and media bytes. `probe` checks image/video/audio metadata needed by preflight; `video` supplies decoder sessions; `audio_graph` prepares and executes tracks, effects and master analysis; `sink` accepts rendered frames and encodes them; `output` finalizes and publishes the result.

The crate uses `ffmpeg-next` for native media probing, video decoding and
software scaling. Encoding and audio processing use the separate `ffmpeg`
executable; runtime probing does not invoke `ffprobe`. The native dependency
builds bundled FFmpeg libraries from source; wheel users do not need the C/Rust
build toolchain. A source build can fail when its compiler, NASM or source fetch
is unavailable, while encoding or audio execution can fail when `ffmpeg` or a
required codec is unavailable.

## From paths to prepared media

Preflight resolves the project-relative asset paths once, probes relevant media, and records dimensions/durations for plan compilation. Preparation decodes static resources, creates video decoder sessions and produces master-audio analysis when visuals need it. During rendering, the SDK supplies rendered frames to the frame sink while the audio graph produces the audio execution output. The media layer, not a renderer, owns muxing and encoded-output lifetime.

### Audio duration

Audio probing decodes the first audio stream through the linked FFmpeg libraries
and divides its decoded per-channel sample count by its sample rate. This matches
the stream and sample-based trimming used by the audio graph. Container duration,
other streams, start timestamps and timestamp gaps do not extend the audio's
source-local duration. Missing or estimated duration metadata therefore needs no
container fallback. Codec delay and padding follow FFmpeg's decoded output, so
the result may differ slightly from a container's declared duration.

Probing reads the whole source, with packet/frame storage bounded independently
of source length; long audio increases validation and preparation time. Empty
audio, missing audio streams, reported demux/decode errors, corrupt packets/frames,
and zero or changing decoded sample rates produce media diagnostics. Native
audio paths must be valid UTF-8 without NUL bytes; unsupported paths are reported
as errors rather than passed to the bindings' panicking path conversion.

## Frame writes and publication

`FrameSink::write_frame(frame_number, rgba)` borrows an RGBA byte slice for one
write. The SDK retains the renderer's owned frame and supplies its number/bytes;
media has no renderer type or crate dependency. `FfmpegSink` validates output
order and expected buffer length before writing FFmpeg stdin. The SDK owns
completion ordering, cancellation and scoped encoder-write overlap.

Frame writing, encoder finalization and publication are deliberately separate. The sink writes a temporary target, finalizes the container, then `output` publishes it at the requested destination subject to overwrite policy. Each stage can fail and cleanup preserves the original destination where possible. Callers should use the stage, temporary path and destination in `RenderFailureContext` rather than assuming that "encoding completed" means the output exists.

For frame selection, caching and concurrency contracts, see the [media pipeline](../media-pipeline.md).
