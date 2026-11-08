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

Ordinary inspection and reusable preparation open the first audio stream through
linked FFmpeg libraries and use its positive stream duration when available.
`AudioMediaInfo::duration_source` distinguishes **reported** metadata from
**decoded** sample duration; neither container duration nor another stream can
extend the selected audio. Missing or invalid stream duration falls back to full
decoding. Opening the decoder also rejects unsupported codecs. Explicit source
bounds beyond the reported duration trigger decoding before rejecting the range,
so underestimated metadata does not reject a valid selection.

Reported duration is provisional: it cannot prove sample availability, integrity,
or a constant sample rate. Preflight/preparation reports include
`VESTRA-AUDIO-DURATION-REPORTED` for each such asset; inspected audio clips expose
`duration_source: "reported" | "decoded"`. Omitted trim ends and automatic
prepared timelines use the chosen duration. For exact duration and full-stream
decoder checks, select `PreflightOptions::with_strict_audio(true)` or
`PrepareOptions::with_strict_audio(true)`; Python accepts `strict_audio=True`, and
CLI validation/inspection accept `--strict-audio`. Pure project validation still
performs no I/O.

Strict probing counts decoded per-channel samples and divides by sample rate.
It rejects reported demux/decode errors, corrupt packets/frames, empty audio, and
zero or changing decoded sample rates. Codec priming/padding follows FFmpeg's
decoded output, which may differ from metadata. Full decoding establishes the
samples actually returned by the decoder, not a checksum or proof that a source
was never truncated before it reached Vestra.

### Audio execution and discrepancies

One-shot rendering resolves decoded durations before compilation, preserving
sample-based omitted trim ends, automatic output duration and AAC padding from
#109. This intentionally retains full-source probing cost during rendering;
metadata is sufficient for inspection, but cannot establish render correctness.

A reusable project prepared from metadata retains its fixed prepared timeline.
Before audio analysis or video encoding, Vestra fully decodes each participating
source once and checks that its selected samples exist (one 48 kHz sample is
allowed for edge rounding). Early EOF or decode errors fail before output
creation; mixer padding must not conceal missing source samples. Surplus decoded
samples lie outside the prepared trim and are discarded. Prepare with strict
audio to include the exact source end initially. Explicit bounded excerpts that
fit the decoded source remain valid even if unused metadata is misleading.

After sample trimming, the audio graph rebuilds contiguous source-local timestamps
before effects and authored timeline placement. Nonzero timestamp origins and
source gaps cannot discard samples or shift authored silence. Source trim,
playback speed and output synchronization retain the same execution pipeline.
Keep prepared sources available and unchanged throughout their lifetime.

Successful and failed probes are reused by resolved path within an operation;
subsequent preflight/preparation operations probe again. There is no persistent
cross-operation cache. The first audio-consuming operation on a provisional prepared snapshot verifies
its sources; subsequent outputs reuse that verification under the same unchanged-
source lifetime rule as strict preparation. Visual-only outputs without analysis
do not need an audio verification scan. Metadata inspection retains one input/decoder;
full decoding additionally retains one packet/frame at a time. FFmpeg's own
buffers and indexes depend on format.

One-shot cancellation is checked between assets and native packet/frame operations,
as is prepared video's verification. A native open/read/decode already in progress
must return first; stalled filesystem I/O cannot be preempted. Standalone
validation/preparation remain synchronous. Native paths must be valid UTF-8
without NUL bytes; failures are diagnostics rather than panics.

Packet counts, limited decode windows and seeking cannot establish full-stream
integrity or retained AAC samples. Parallel probing and persistent caches are not
required for the measured metadata path; they would add resource limits and file
invalidation policy without improving its duration guarantees. See the focused
[benchmark procedure](https://github.com/evgen2571/vestra/blob/develop/benchmarks/README.md).

## Frame writes and publication

`FrameSink::write_frame(frame_number, rgba)` borrows an RGBA byte slice for one
write. The SDK retains the renderer's owned frame and supplies its number/bytes;
media has no renderer type or crate dependency. `FfmpegSink` validates output
order and expected buffer length before writing FFmpeg stdin. The SDK owns
completion ordering, cancellation and scoped encoder-write overlap.

Frame writing, encoder finalization and publication are deliberately separate. The sink writes a temporary target, finalizes the container, then `output` publishes it at the requested destination subject to overwrite policy. Each stage can fail and cleanup preserves the original destination where possible. Callers should use the stage, temporary path and destination in `RenderFailureContext` rather than assuming that "encoding completed" means the output exists.

For frame selection, caching and concurrency contracts, see the [media pipeline](../media-pipeline.md).
