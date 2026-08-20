# Phase 9C: clip gain automation and equal-power crossfades

Phase 9C keeps schema version 2. `AudioClip.gain_automation` is optional and
contains an ordered `keyframes` array. A keyframe has `time`, `gain`, and an
outgoing `interpolation`: `linear` or `hold`. Times are clip-local playback
seconds after source trimming. The first point is exactly zero, later points
strictly increase, and time and gain are finite and nonnegative. Gain is a
linear multiplier. Zero silences, one is unity, values above one amplify, and
the final point holds through selected clip end.

The effective clip multiplier is `clip.gain * automation(t) * fade(t)`. Track
gain remains after track mixing. Automation does not change project duration.
The project permits 16,384 gain keyframes in total. The boundary is inclusive;
one additional point reports `MVP-LIMIT-AUDIO-GAIN-KEYFRAMES`, including for
muted clips and audio-disabled output.

Core validation rejects empty automation, a nonzero first point, unordered or
duplicate times, and invalid gains. Preflight rejects the last point when it
is later than the resolved selected source duration. Explicit and implicit
source-end boundaries both have regression tests.

`AudioMixPlan` retains automation and both fade curves as logical data. It
does not contain FFmpeg syntax. Inspection reports static gain, ordered
automation points, and both curve fields. Python inspection exposes the same
data through immutable native DTOs.

The media graph uses FFmpeg `aeval` for every 48 kHz mixer sample. Before graph
generation it rounds automation times through the same public
`video_editor_media::seconds_to_samples` policy used for trim and placement.
`volume` only applies static gains because FFmpeg evaluates its expression once
or per audio frame. The graph restores an explicit `fltp` stereo layout after
`aeval`, which is required by the AAC encoder.

Gain automation dispatch is a balanced binary `if(lt(t, boundary), left,
right)` tree. Its leaves are the ordered linear or hold segments followed by the
post-final-keyframe hold. This keeps FFmpeg conditional nesting `O(log N)` and
builds one expression by appending to a single `String`, rather than repeatedly
copying an already-built right-deep expression. It preserves authored order and
does not sort, merge, or approximate keyframes. Focused tests compile identical
expressions twice, run 100 and 1,000 keyframes through FFmpeg's parser using
the production file-indirection option, and run the configured 16,384-keyframe
boundary through that parser. A 128-keyframe PCM render verifies the compiled
graph still changes real signal amplitude.

Canonical keyframe times remain seconds, but execution is sample-quantized.
Preflight uses `seconds_to_samples`, which rounds `seconds * 48_000` to the
nearest mixer sample. Each successive keyframe must resolve to a strictly later
sample. Otherwise preflight reports
`MVP-AUDIO-AUTOMATION-SAMPLE-RESOLUTION` at the later keyframe, explaining that
it resolves to the same 48 kHz sample as the preceding point. This rule applies
to both linear and hold segments. One full sample separation is accepted;
sub-sample pairs at zero and at a nonzero boundary are rejected. Graph
compilation repeats the check defensively for manually constructed plans.

Linear fades remain the default. Equal-power fade-in is `sin(pi*u/2)` and
fade-out is `cos(pi*u/2)`, where `u` is normalized fade progress. The PCM test
measures rising and descending linear ramps at 25% and 75%, a hold change at
the quantized 0.12345 s boundary, the equal-power midpoint near `1/sqrt(2)`, and the multiplicative
track gain * clip gain * automation gain result of 0.125. The same PCM test
checks the linear fade midpoint at 0.5, proving the two curves differ.
An independent 440 Hz and 880 Hz crossfade test measures each component at the
midpoint near `1/sqrt(2)` and keeps the squared-gain sum within 0.06 of one.

Python exposes `AudioGainKeyframe`, `AudioGainInterpolation`, and
`AudioFadeCurve`. `AudioClip.set_gain_automation()` validates before replacing
an immutable tuple; caller list mutation cannot change clip state.
`AudioTimeline.crossfade(outgoing, incoming, *, curve=AudioFadeCurve.EQUAL_POWER)`
uses the full existing overlap. It requires an earlier outgoing clip with an
explicit `trim_end`, does not move clips or alter trims, rejects a gap,
unresolved outgoing end, foreign builder handles, or conflicting fades, and
commits the two fade changes only after all checks pass. It writes ordinary
clip fade fields, never a canonical `AudioCrossfade` relation.
An existing nonzero fade must match both the overlap duration and requested
curve exactly; a matching-duration linear fade is not silently converted to
equal power.

Raw PCM tests remain the mathematical authority for exact automation and
equal-power envelopes. The production AAC test decodes the finished MP4 to
48 kHz stereo `f32le`. It renders a 440 Hz outgoing clip and an 880 Hz incoming
clip with a 0.5-second equal-power overlap plus incoming hold automation at a
non-millisecond 0.12345-second boundary. Codec-tolerant windows verify 440 Hz
dominates early, both tones occur around the midpoint, 880 Hz dominates late,
and the incoming tone becomes measurably louder after automation changes. AAC
cannot preserve sample-exact envelope values, so raw PCM remains the precise
mathematical proof.

The clean-wheel smoke installs the built CPython 3.13 wheel into an isolated
virtual environment outside the repository. It imports from `site-packages`,
checks `py.typed`, builds, validates, and inspects a two-track public-authoring
automation/crossfade project, prepares CPU rendering, renders MP4/AAC, and
decodes 770,048 bytes of nonempty `f32le` PCM.

Final gates passed on CPython 3.13.5 with FFmpeg and FFprobe 7.1.5: `cargo fmt
--all -- --check`, `cargo check --workspace`, strict workspace Clippy, and
`cargo test --workspace`; schema validation; Maturin development installation
and wheel build; the full Python suite (252 passed, 4 adapter-gated WGPU skips);
strict mypy; stubtest; and the isolated-wheel smoke. Existing Phase 9B tests
continue to cover source limits, graph-file cleanup, cancellation, missing
operation-time sources, and repeated prepared renders. Phase 9D remains
deferred.

Phase 9D remains deferred: track/master automation, routing, dynamics,
analysis, spectral features, PCM caching, and real-time audio.
