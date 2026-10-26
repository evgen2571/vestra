# Media pipeline and concurrency

Vestra separates project meaning, pixels, and media I/O. The SDK coordinates
these owners; CPU and WGPU consume the same compiled timeline.

```text
media source → probe/decode → frame selection/cache
                                      ↓
canonical project → validate/preflight → prepared project
                                      ↓
                              evaluate → renderer
                                      ↓
                               ordered RGBA frames
                                      ↓
                            encoder → finalize → publish
```

## Preparation and ownership

Semantic validation in `vestra-core` does not touch files or devices. SDK
preflight resolves project-relative paths, probes media, checks the operation's
resources and output policy, and supplies media facts to compilation.
`vestra-media` owns probing, native FFmpeg decoding, audio execution, encoding,
and publication. `vestra-render` owns CPU/WGPU rendering resources.

A prepared project retains the compiled plan, static image/text resources,
audio analysis, renderer state, and video decoder sessions. Individual frame
and video operations reuse that preparation. Video remains an external resource:
a snapshot is not a copy of the original footage. Keep source files available
and unchanged while using prepared state.

## Video frame selection

Video requests use source-media time derived from clip placement, trim and
playback rate. The native decoder selects the latest presentation timestamp at
or before the request, respecting the stream time base and source origin.
Advancing requests decode intervening native frames but convert only the
selected frame to RGBA. Backward requests seek to a keyframe and flush decoder
state. Do not substitute nominal frame rate for presentation timestamps;
variable-frame-rate and B-frame footage require timestamp-based selection.

The cursor retains its selected image and native lookahead. Repeated requests
within that image's presentation interval reuse it even when optional RGBA
caching is disabled. At EOF, the final frame can be held. Returned RGBA frames
own their storage; the decoder can safely advance afterward.

Decoder EOF holding does not authorize an authored clip to exceed its media:
preflight currently checks the full authored source span, even when output
clipping would shorten execution. Diagnose those as separate boundaries; see
[timeline validation authority](architecture/project-to-plan.md#duration-resolution-and-validation-authority).

The optional LRU cache is bounded by retained RGBA bytes. A cache entry records
its observed successor timestamp: neighbouring cache entries alone cannot prove
coverage, because sparse requests and seeks leave gaps. The current image,
native frames, scaler allocation and decoder contexts are separate bounded
resources, outside the optional cache budget. Cache counters are not total
process memory.

## CPU decoder reuse

Each CPU worker has independent state. Its video pool keeps one cached primary
cursor per asset across serial clips and idle frames. Within a frame, equal
asset/time requests share a cursor. Concurrent placements of the same asset at
different times use additional uncached cursors; unused extra cursors are
retired after the frame. Draw order can change which cursor follows a timeline
and may require a seek.

This balances serial reuse with simultaneous timelines without assigning a
persistent decoder to every historical clip. More workers can improve frame
throughput while increasing decoder contexts, repeated native decoding, scratch
storage and retained memory. Measure those costs together; decoder throughput
and end-to-end wall time are different quantities.

## WGPU decode and prefetch

WGPU gathers video reads from evaluated work, including dependencies needed by
masks and track mattes. Hidden unused video must not be decoded, but hiding a
matte source does not remove its consumer's need for coverage. Compiled video
slots distinguish separate placements and nested timelines.

Native decoder sessions run on owner threads. WGPU enables a session hint that
permits one additional native frame to be decoded after a successful reply.
Queued demand takes priority. At most two pending native frames are retained;
no speculative RGBA conversion or timestamp prediction occurs. Ordinary CPU
sessions leave this prefetch disabled.

Prefetched errors surface in decode order when requested; seeking clears queued
frames and errors. Statistics synchronize with speculative work, and teardown
joins the owner thread. One-frame lookahead bounds work count and storage,
not decode latency: a native decode may read multiple packets.

## Audio execution and analysis

Audio clips are trimmed, processed in ordered clip/track/master effect stacks,
and mixed on the project timeline. Fades and gain automation use their documented
time domains. Prepared master audio supplies RMS, peak and frequency-band
signals and Spectrum2D analysis; analysis is independent of whether audio is
published in the final output.

Analysis reuses FFT scratch within a preparation operation. Analysis timings,
audio execution and video frame work have separate scopes. See the
[audio contract](../reference/audio.md) and [signals](../reference/signals.md).

## Ordered frames and encoder overlap

The staged coordinator bounds backend submissions and reorders completions
before writing frames. CPU workers compose concurrently. Dynamic WGPU rendering
can write one ordered frame on a scoped thread while the caller evaluates and
submits at most one further frame into a free backend slot. The writer borrows
the existing frame and sink; there is no persistent encoder queue or pixel copy.
Static rendering uses a separate reuse path.

Each write is joined before another write, progress delivery, cancellation
cleanup, encoder finalization or publication. Successful writes are acknowledged
before reporting a speculative submission error. A write error takes precedence
when both operations fail; cancellation from progress delivery takes precedence
over a speculative error. The transient combined frame-payload bound is twice
backend capacity, including the writer's frame. Pipeline depth is an internal
resource control, not a project semantic setting.

Encoder-write timing can overlap frame work. Do not sum concurrent stage times
to infer wall time or assume a deeper pipeline is always faster.

## Completion and failure

Rendering the last frame does not publish a video. The sink finishes the encoder
and temporary container, then publishes the destination subject to overwrite
policy. `completed` and a successful `RenderResult` occur only after publication.
With overwrite disabled, successful hard-link creation commits the destination;
failure to remove its temporary name is logged without reporting publication
failure for an already-published output.
Cancellation and failures drain/join owned work and clean up temporary output;
reports identify the stage, temporary path and destination where available.

Use [render lifecycle](../concepts/rendering-lifecycle.md) for the public contract,
[render pipeline](architecture/render-pipeline.md) for SDK ownership, and
[performance](performance.md) for measurements. Correctness checks should include
sparse/backward/VFR frame requests, cache boundaries, same-asset offsets,
mask/matte dependencies, ordered completion, cancellation, and encoder failures.

## Verify encoded output

Reuse generated moving-frame and audio-marker fixtures from the media/SDK tests
and benchmark workloads. Fix source dimensions, rational frame rate, seeds and
audio placement; use lossless input where exact source pixels matter. Avoid
downloaded footage or new fixture generators when existing helpers cover the case.

Probe the published target, not just the encoder's temporary file. For example:

```bash
ffprobe -v error -select_streams v:0 -count_frames \
  -show_entries stream=codec_name,pix_fmt,width,height,r_frame_rate,avg_frame_rate,time_base,start_time,duration,nb_read_frames \
  -of json target/media-check/output.mp4
```

Compare decoded frame count with the resolved plan, not just `nb_frames` or
rounded container duration. Inspect `-show_frames`/`-show_packets` timestamps
when diagnosing drift, discontinuities or missing tails. Compare presentation
order: B-frame PTS and packet DTS need not be equal. Decode audio markers at
early/middle/final positions to measure AV alignment, allowing justified codec
delay and container/sample rounding. Matching stream durations alone cannot
prove sync. Raw frame fidelity and encoded-media correctness are separate checks.

Current output is MP4 with libx264 H264/yuv420p; quality controls CRF and an
internal x264 preset. Hardware WGPU rendering does not imply hardware decode,
hardware encoding or direct GPU-to-encoder transfer. Record actual renderer,
adapter and encoder separately. Selected-range video, resumability and alternate
output codecs remain proposals rather than established test targets.

For long-duration/large-file tests, state actual timeline length, frame count,
encoded bytes, disk budget and timestamp coverage. Current default timeline
caps still apply; low-cost sparse boundary tests do not prove sustained encoding
or >4 GiB container behavior. Keep expensive runs explicit and outputs temporary;
combine with [sustained resource profiling](performance.md#allocation-and-sustained-render-profiling).

Use existing sink/publication fault tests for decode, write, finalization,
publication and cancellation failures. Check stage/error context, joins and
temporary cleanup, and that a prior good destination survives failed work.
Only successful publication permits terminal success; the callback/cancellation
edge cases are defined by [render lifecycle](../concepts/rendering-lifecycle.md).
