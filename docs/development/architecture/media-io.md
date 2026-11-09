# Media I/O

`vestra-media` owns every transition between Vestra values and media bytes. `probe` checks image/video/audio metadata needed by preflight; `video` supplies decoder sessions; `audio_graph` prepares and executes tracks, effects and master analysis; `sink` accepts rendered frames and encodes them; `output` finalizes and publishes the result.

The crate uses `ffmpeg-next` for native media probing, video decoding and
software scaling. Encoding and audio processing use the separate `ffmpeg`
executable; Python supplies an absolute path selected from `imageio-ffmpeg` or
its explicit override, while Rust/CLI callers default to PATH. Runtime probing
does not invoke `ffprobe`. The native dependency
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
the result may differ slightly from a container's declared duration. After sample
trimming, the audio graph rebuilds contiguous source-local timestamps before
effects and authored timeline placement; source timestamp gaps cannot drop audio.

Probing reads the whole source; long audio increases validation and preparation
time. Positive stream metadata is not sufficient to skip decoding: FLAC sample
counts can be wrong, AAC padding can differ from the declared duration, and
metadata or seeking cannot detect errors and sample-rate changes in the middle.
Packet durations also need not describe the samples retained by the decoder.
Preflight reuses each resolved audio path's result within that operation, including
failures. Subsequent operations probe again, so edited files are not hidden by a
persistent cache. Preparing once and reusing the prepared project avoids repeated
preflight; its normal source-lifetime rules still apply.

The probe retains one packet and decoded frame at a time; FFmpeg's own buffers
and indexes depend on the container and codec. One-shot render cancellation is
checked between packets/frames and assets. A native open/read/decode call already
in progress must return before cancellation is observed; stalled filesystem I/O
is not preempted. Standalone validation and preparation remain synchronous.

Empty audio, missing audio streams, reported demux/decode errors, corrupt packets/frames,
and zero or changing decoded sample rates produce media diagnostics. Native
audio paths must be valid UTF-8 without NUL bytes; unsupported paths are reported
as errors rather than passed to the bindings' panicking path conversion.

### Future probing direction

The intended follow-up architecture uses fast native FFmpeg metadata inspection
by default, with optional full decoding for precise duration and strict validation.
It will continue to avoid a runtime `ffprobe` dependency. The implementation above
still fully decodes audio; this direction is a separate change, not a current mode.

That change needs to:

- Represent unavailable or estimated duration and its provenance separately from
  verified decoded duration, and define native fallbacks for missing metadata.
- Carry the selected precision/validation policy through SDK, CLI and Python
  preflight/preparation without silently promising strict checks in fast mode.
- Define how estimates affect automatic timeline length, omitted trim ends and
  explicit source ranges. Keep actual decoded samples authoritative during
  rendering, with explicit diagnostics and a documented policy for early EOF,
  extra samples and discrepancies discovered after compilation or encoding starts.
- Preserve first-stream selection, sample-based trimming, timestamp normalization,
  cancellation and useful malformed-input errors. Full-stream corruption and rate
  checks belong to strict decoding; metadata inspection cannot establish them.
- Test both policies against misleading/missing metadata, padding, multiple streams,
  timestamp discontinuities, truncated media and changed files. Measure short/long
  assets and repeated validation/preparation separately from render throughput.

The current full-decoding cost and reproducible probe benchmark are described in
[benchmarks](https://github.com/evgen2571/vestra/blob/develop/benchmarks/README.md).

## Frame writes and publication

`FrameSink::write_frame(frame_number, rgba)` borrows an RGBA byte slice for one
write. The SDK retains the renderer's owned frame and supplies its number/bytes;
media has no renderer type or crate dependency. `FfmpegSink` validates output
order and expected buffer length before writing FFmpeg stdin. The SDK owns
completion ordering, cancellation and scoped encoder-write overlap.

Frame writing, encoder finalization and publication are deliberately separate. The sink writes a temporary target, finalizes the container, then `output` publishes it at the requested destination subject to overwrite policy. Each stage can fail and cleanup preserves the original destination where possible. Callers should use the stage, temporary path and destination in `RenderFailureContext` rather than assuming that "encoding completed" means the output exists.

For frame selection, caching and concurrency contracts, see the [media pipeline](../media-pipeline.md).
