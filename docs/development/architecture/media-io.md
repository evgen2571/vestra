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

Inspection opens the first audio stream through linked FFmpeg libraries and uses
its positive stream duration when available. `AudioMediaInfo::duration_source`
and inspected clips distinguish **reported** metadata from **decoded** sample
duration. Metadata use alone emits no warning. Inspection falls back to full
decoding when duration is missing; container duration and other streams never
extend the selected audio.

One-shot and reusable preparation share the same execution policy. An explicit
`trim_end` defines a bounded selection without needing source duration. Its
source metadata may be unavailable or inaccurate; actual reading verifies the
output-visible selection. Preparation does not scan merely because a bound
exceeds reported metadata. An omitted `trim_end` requires exact EOF resolution
before compilation, including AAC decoded padding. This also applies to muted
or disabled-output audio because it contributes structural automatic duration.

Exact open-ended timelines fundamentally require reaching EOF: a FLAC reporting
10 seconds may decode to 0.375 seconds. Using metadata would change frame counts,
end-relative fades and automatic duration. Late recompilation would invalidate
prepared frame access and resources. Therefore open-ended preparation retains a
full scan; author an explicit trim end for bounded preparation and rendering.

Select `PreflightOptions::with_strict_audio(true)` or
`PrepareOptions::with_strict_audio(true)` to audit all complete audio sources.
Python accepts `strict_audio=True`; CLI validation/inspection accept
`--strict-audio`. Strict decoding counts per-channel samples, rejects reported
demux/decode errors, corrupt packets/frames, empty audio and zero/changing sample
rates. It establishes decoded sample duration, not a checksum or proof that a
source was never truncated before Vestra received it. Pure project validation
still performs no I/O.

### Audio execution and discrepancies

The production FFmpeg audio graph shares each source input across clip branches.
A named `astats` counter after 48 kHz normalization records samples consumed by
that same read. Bounds checks use the output-visible prefix, mapping clip speed
transforms back to source samples; logical clip durations, fades and automation
remain unchanged. Reading is sequential, preserving priming and source-local
sample trimming across nonzero/gapped timestamps. Later offsets may therefore
require decoding the preceding prefix. Codec/filter lookahead may read beyond
the exact requested endpoint.

Source counters are checked after analysis or encoder finalization and before
accepting features or publishing output. Padding cannot conceal unavailable
samples. A missing, malformed or reset counter fails closed. Bounds allow one
48 kHz sample of end rounding, but starts rounding to EOF fail. Errors clean up
temporary output and preserve an existing destination. Counters describe consumed
coverage only: corruption beyond that region may remain unexamined, while strict
validation audits the entire source. Prefix coverage never becomes `decoded`
duration provenance.

The counter uses FFmpeg's named `astats` summary format, retained independently
of the bounded diagnostic tail. This implementation dependency is covered by
parser tests and actual native CI, including Windows/macOS. It avoids a separate
verification decode, PCM spooling and duration-proportional cached storage.
Repeated outputs read the needed samples again; clips sharing a path reuse the
same decoder within each graph. Keep prepared sources unchanged and available.

After trimming and authored delay, and after final master padding, the graph
rebuilds contiguous sample-based timestamps. One-shot, default-prepared and
strict-prepared output agree for valid selections; strict validation can still
reject corruption outside a bounded ordinary render's examined region.

Successful and failed native probes are reused by resolved path within preflight;
subsequent operations observe source edits. Metadata inspection retains an input
and decoder; exact scanning additionally retains one packet/frame at a time.
FFmpeg buffers and indexes depend on format. There is no persistent global cache.

Rendering cancellation still aborts/reaps the owned encoder and cleans temporary
files. Native exact scans check cancellation between assets and packet/frame
operations; stalled filesystem I/O cannot be preempted. Standalone preparation
and validation remain synchronous. Native paths must be UTF-8 without NUL bytes;
unsupported paths produce diagnostics.

See the [benchmark protocol and results](https://github.com/evgen2571/vestra/blob/develop/benchmarks/audio-probing.md)
for preparation-plus-first-render costs, repeated output and memory evidence.

## Frame writes and publication

`FrameSink::write_frame(frame_number, rgba)` borrows an RGBA byte slice for one
write. The SDK retains the renderer's owned frame and supplies its number/bytes;
media has no renderer type or crate dependency. `FfmpegSink` validates output
order and expected buffer length before writing FFmpeg stdin. The SDK owns
completion ordering, cancellation and scoped encoder-write overlap.

Frame writing, encoder finalization and publication are deliberately separate. The sink writes a temporary target, finalizes the container, then `output` publishes it at the requested destination subject to overwrite policy. Each stage can fail and cleanup preserves the original destination where possible. Callers should use the stage, temporary path and destination in `RenderFailureContext` rather than assuming that "encoding completed" means the output exists.

For frame selection, caching and concurrency contracts, see the [media pipeline](../media-pipeline.md).
