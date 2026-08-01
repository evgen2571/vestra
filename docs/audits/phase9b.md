# Phase 9B audit

The Phase 9B media path takes `AudioMixPlan` directly. `AudioSettings`,
`single_input_settings`, and the `MVP-AUDIO-MIX-UNSUPPORTED` bridge are gone.

The working format is 48,000 Hz, stereo, `fltp`. The media graph converts each
audible source with `aformat`, trims with `atrim` sample edges, resets timestamps,
applies clip gain and linear `afade` filters, then places it with sample-suffixed
`adelay`. Track clips sum first, track gain follows that submix, and track
outputs sum at the master. Every multi-input `amix` sets
`duration=longest:dropout_transition=0:normalize=0`.

`seconds_to_samples` uses `round(seconds * 48000)`. It rejects non-finite,
negative, and unrepresentable values. Logical interval edges are rounded before
subtraction. When fade rounding would exceed a selected clip by one sample, the
fade-out is shortened to fit. The maximum timing quantization error is half a
mixer sample before encoder/container effects.

The master uses `apad=whole_len` and `atrim=end_sample` against the resolved
project duration. Audio-disabled and no-audible-contributor operations remain
video-only, so `RenderSummary.audio_present` reports false. External audio files
are not cached by `PreparedProject`; FFmpeg reopens them per operation.

The media tests compile the production graph into raw floating PCM. They verify
non-millisecond sample placement, silence before onset, deterministic input
order, explicit 48 kHz stereo `fltp` normalization, and unnormalized two-track
summation. A second test runs the same multi-input graph through the raw-video
encoder and verifies an AAC, 48 kHz, stereo stream with FFprobe. Python
integration verifies multi-clip rendering,
two repeated prepared-project operations, and video-only output for disabled,
muted, or zero-gain mixes. A missing operation-time source returns FFmpeg's
structured failure and leaves no output. A real multi-input operation cancelled
from its first progress event reaps FFmpeg and removes the temporary and final
outputs.

Phase 9C work remains deferred: automation, crossfades, and all other dynamic
audio controls are out of scope.

## Phase 9B finalization

The graph now deduplicates execution inputs by the resolved `AudioClipPlan`
path. The first audible use in track declaration order then clip declaration
order allocates the next FFmpeg audio input. Raw video remains input 0, so the
first unique audio source is input 1. Asset IDs do not affect this decision.
Muted tracks, muted clips, and zero-gain clips do not register an input.

The executor normalizes each unique source before branching. A source used by
more than one audible clip emits one `aformat` followed by `asplit`; a source
used once skips `asplit`. Branches are consumed in the same declaration order
that registered them. Each branch then has its own `atrim`, timestamp reset,
gain, linear fades, and `adelay`, so reused sources retain independent trims,
gain, fades, and placements. Internal labels come from a monotonic counter and
never use project IDs or paths.

`ResourceLimits::maximum_audio_sources` is a separate execution-resource
contract with a default of 128. It remains configurable through the existing
Rust `ValidationOptions::limits` path, but is not a schema or Python authoring
field. The media executor counts the final deduplicated `input_paths` after it
has discarded muted tracks, muted clips, and zero-effective-gain branches, then
rejects counts above the limit before it adds FFmpeg input arguments or starts
the process. The count uses resolved paths, not canonical asset IDs, clip count,
or `asplit` branches. 128 is deliberately below common process descriptor
limits, leaving room for video stdin, output, stderr/stdout pipes, demuxers,
the graph file, and FFmpeg internals across supported platforms.

Focused tests accept exactly four paths under a limit of four and reject five
with `MediaError::AudioSourceLimit { actual, maximum }`. They also cover 256
clips over four sources and muted or zero-gain-only paths, confirming that only
the one audible path consumes the execution budget.

Graphs larger than 64 KiB of UTF-8 filter text use FFmpeg 7+'s
`-/filter_complex <file>` indirection; smaller graphs retain
`-filter_complex`. `ffmpeg -h full` on the supported FFmpeg 7.1.5 reports
`-filter_complex_script` as deprecated in favor of this option, so the
deprecated option no longer appears in the execution path. The file is created
beside the operation output with a UUID name, after graph
compilation and before process spawn. The sink owns it through completion,
failure, cancellation, and drop, then removes it. Paths remain structured
FFmpeg input arguments and never enter filter syntax.

The real media integration test builds an 87,883-byte graph with 512 clips and
one unique WAV source, asserts that it crossed the 64 KiB threshold, starts
FFmpeg through the file-backed path, writes raw video, produces a non-empty
MP4 with an AAC stream, and confirms the temporary graph file disappears after
successful completion. Companion large-graph tests force a missing input and
an explicit cancellation after FFmpeg starts, then confirm the same temporary
file is removed and no final output is published.

The production AAC integration now renders a two-track 440 Hz and 880 Hz mix,
reuses the 440 Hz source for a second placement, decodes the final MP4 AAC
stream with FFmpeg to interleaved stereo `f32le`, and checks both frequencies
in an overlap window. It also checks that the pre-onset region is at least ten
times quieter by RMS. This deliberately uses broad codec-level windows, not
the raw PCM suite's sample-accurate placement contract.

## Verification record

Before finalization, the graph opened one FFmpeg input for each audible clip.
Repeated assets therefore opened once per use. The finalized graph opens one
input per unique audible resolved path. Filter labels use internal counters and
never contain user IDs.

The PCM harness runs the production graph compiler and decodes its result as
raw `f32le`. It covers nearest-sample placement at 0.12345 seconds, silence
before onset, trim identity, clip gain, linear fades, same-track summation,
track gain after the submix, two-frequency coexistence, silent gaps, project
end trimming, and 44.1 kHz mono plus 48 kHz stereo normalization. The tested
amplitude tolerance is 0.002. Mono to stereo conversion uses FFmpeg's
equal-power coefficients, so the unnormalized two-branch check expects about
0.141421 per output channel.

The AAC integration test sends raw RGBA video and a multi-input mix through
`FfmpegSink`, then FFprobe confirms `codec_name=aac`, `sample_rate=48000`, and
`channels=2`. The public Python test builds a two-track mix, checks
`audio_present`, renders a prepared project twice, verifies video-only cases,
and cancels a real multi-input operation. A clean CPython 3.13 wheel install
outside the repository built and rendered a two-track overlapping mix with an
AAC, 48 kHz, stereo stream.

Executed gates: `cargo fmt --all -- --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace`, `maturin develop`, the full Python suite, mypy,
stubtest, schema validation, and `maturin build`. The latest full Python run
reported 241 passed and 4 adapter-gated WGPU skips. FFmpeg and FFprobe were
both 7.1.5. The WGPU skips are environmental and unrelated to audio.

Finalization reran `cargo fmt --all -- --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
`cargo test --workspace`, plus focused tests for every workspace package and
the public SDK/export suites. The Python gates passed in the project CPython
3.13.5 virtualenv: 241 passed, 4 adapter-gated skips, clean mypy, clean
stubtest, and schema validation. `maturin develop` and `maturin build` passed.
An isolated CPython 3.13 wheel install outside the repository imported from
site-packages and contained `py.typed`. Its smoke built, validated, inspected,
prepared, and rendered an MP4/AAC project with two tracks and overlapping
reused audio. FFprobe confirmed AAC, 48 kHz, stereo and FFmpeg decoded 385,024
bytes of PCM. The four Python WGPU skips are adapter-gated environment limits;
the CPU and audio paths passed.

## Closure status

Phase 9B static mixer execution is complete. Phase 9C remains explicitly
deferred: no automation, crossfade helpers, routing, pan, mastering, playback,
or other dynamic audio features were added in this closure pass.
