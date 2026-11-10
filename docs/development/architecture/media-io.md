# Media I/O

`vestra-media` probes and decodes inputs, executes audio, encodes rendered
frames and publishes output. Renderers supply pixels; media owns files,
containers and encoder lifetime.

| Module | Responsibility |
| --- | --- |
| `probe` | Image/video facts and decoded audio duration for preflight. |
| `video` | Reusable video decoder sessions. |
| `audio_graph` | Tracks, effects, mixing and master-audio analysis. |
| `sink` | Frame writes and encoding. |
| `output` | Output finalization and publication. |

The crate uses `ffmpeg-next` for native media probing, video decoding and
software scaling. Encoding and audio processing use the separate `ffmpeg`
executable. Python selects an absolute path from `imageio-ffmpeg` or
`IMAGEIO_FFMPEG_EXE`; Rust and the CLI default to PATH. Probing does not invoke
`ffprobe`. See [installation](../../getting-started/installation.md) for binary
and source-build requirements.

## From paths to prepared media

Preflight resolves project-relative asset paths, probes media and supplies
dimensions and durations to plan compilation. Preparation then decodes static
resources, creates video decoder sessions and produces master-audio analysis
when visuals need it. During rendering, the SDK supplies frames to the sink
while the audio graph produces the audio output.

### Audio duration

Audio duration is the first audio stream's decoded per-channel sample count
divided by its sample rate. Container duration, other streams, start timestamps
and timestamp gaps do not extend it. Codec delay and padding follow FFmpeg's
decoded output, so duration can differ slightly from container metadata.

The audio graph uses the same stream and sample-based trimming. After trimming,
it rebuilds contiguous source-local timestamps before effects and timeline
placement, so source timestamp gaps cannot drop audio.

Probing reads the whole source to measure retained samples and detect midstream
errors or sample-rate changes. Long audio therefore increases preflight time.
Each preflight reuses results, including failures, for assets sharing a resolved
path. Later operations probe again to observe edited files. Reusing a prepared
project avoids repeated preflight; its source-lifetime rules still apply. The
[probe benchmark](https://github.com/evgen2571/vestra/blob/develop/benchmarks/README.md)
measures this cost separately from rendering.

The probe retains one packet and decoded frame at a time; FFmpeg's own buffers
and indexes depend on the container and codec. One-shot render cancellation is
checked between packets/frames and assets. A native open/read/decode call already
in progress must return before cancellation is observed; stalled filesystem I/O
is not preempted. Standalone validation and preparation remain synchronous.

Empty audio, missing audio streams, reported demux/decode errors, corrupt packets/frames,
and zero or changing decoded sample rates produce media diagnostics. Native
audio paths must be valid UTF-8 without NUL bytes; unsupported paths produce
errors.

## Frame writes and publication

`FrameSink::write_frame(frame_number, rgba)` borrows an RGBA byte slice for one
write. The SDK retains the renderer's owned frame and supplies its number/bytes;
media has no renderer type or crate dependency. `FfmpegSink` validates output
order and expected buffer length before writing FFmpeg stdin. The SDK owns
completion ordering, cancellation and scoped encoder-write overlap.

The runtime destination selects MP4, GIF or animated WebP. GIF streams a local
palette per frame and uses full-frame disposal; it does not retain an entire
timeline for palette generation. WebP streams independently encoded lossless
VP8L frames from FFmpeg's `libwebp` encoder into a media-owned RIFF writer.
That writer copies bounded chunks directly to the temporary file, writes
full-canvas no-blend `ANMF` frames, and patches the RIFF length on completion.
It is joined before publication or cleanup. This avoids retaining animation
state in `libwebp_anim`. Container clocks, size limits and transparency policies
are documented in the [output reference](../../reference/render-outputs.md#animation-formats).

The sink writes a temporary target and finalizes the container; `output` then
publishes it under the requested overwrite policy. Each stage can fail, and
cleanup preserves the original destination where possible. `RenderFailureContext`
records the failed stage, temporary path and destination. Successful frame
encoding alone does not mean the output is available.

For frame selection, caching and concurrency contracts, see the [media pipeline](../media-pipeline.md).
