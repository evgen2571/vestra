# Media pipeline and concurrency

Retained changes cover native frame selection, CPU cursor reuse, audio FFT
scratch reuse, GPU video culling, and bounded native prefetch for WGPU.
Decoder-access, buffering, and worker-count experiments are also recorded
below. WGPU asynchronous encoder feeding is retained after performance,
output, and repository verification. Every requested
area has implementation or experimental evidence in the coverage table below;
rejected variants and hardware-specific limitations remain explicit.

For the current disposition of each area, start with the
[coverage summary](#coverage-and-current-status). The surrounding sections
retain the measurements and decisions behind it.

## Native frame selection candidate

The baseline converts every decoded frame to RGBA, including frames passed over
while advancing a worker's video cursor. Eight CPU workers have independent
decoder sessions. In the canonical three-video workload, the baseline performed
4,144 native decodes for 270 frame requests. Each of those decodes also paid for
RGBA conversion and copying.

The candidate selects the latest presentation timestamp at or before the request
in native FFmpeg frames. It converts only the selected frame, retains one native
lookahead, and reuses the scaler's output allocation. Returned images still own
their pixel storage. The decoder retains its current presentation image even
when the optional frame cache has a zero-byte budget, avoiding a seek for a hold
within that image's presentation interval.

The cache stores requested frames and their observed successor timestamp.
Neighbours in the cache do not establish coverage: skipped frames and seeks can
leave gaps. A backward request for an unrequested frame can therefore require a
seek that the former eager-conversion cache avoided. Backward seeking still uses
FFmpeg's keyframe seek and decoder flush.

The cache budget covers retained RGBA cache payloads. Current-frame ownership,
native lookahead, and reusable scaler output are bounded cursor resources outside
that budget. Separate executable-only resource runs measured the resulting
process-memory tradeoff below.

## First paired CPU measurement

Baseline revision: `e16a54a05f8e54b64349e6f9e73929b0178c0bc4`.
Candidate patch is saved with the raw results. Both runs used the canonical
suite definition, 1280×720, one warmup and five samples, a new editor/preparation
per sample, CPU rendering, and the existing FFmpeg output settings. The host is
an AMD Ryzen 5 3600 with 12 logical threads, WSL2 Linux, Rust 1.97.1, and the
FFmpeg 8.0.1 CLI. Full tool and environment details are in the raw records.

| Workload | Baseline wall median, ms | Candidate wall median, ms | Wall change | Aggregate decode-time change |
| --- | ---: | ---: | ---: | ---: |
| Single video | 1,322 | 1,243 | −6.0% | −50.0% |
| Mixed dynamic | 1,919 | 1,815 | −5.4% | −58.3% |
| Three-video heavy | 5,982 | 4,855 | −18.8% | −44.8% |
| Production edit | 9,785 | 9,128 | −6.7% | −33.1% |

Other canonical workloads had wall-median changes between −0.5% and +2.1%.
Decode time sums work across workers and must not be read as wall time. These
are first-run results, not a repeatability or statistical-significance claim.

The second candidate run reproduced the wall medians: 1,248 ms single-video,
1,807 ms mixed-dynamic, 4,867 ms video-heavy, and 9,093 ms production-edit.
Against the original baseline these are −5.6%, −5.8%, −18.6%, and −7.1%.
Aggregate decode-time reductions were 52.5%, 63.9%, 44.4%, and 34.2%.
The second baseline run completed at 1,333, 1,945, 6,060, and 9,844 ms for
those workloads. Comparing the second pair gives wall reductions of 6.4%,
7.1%, 19.7%, and 7.6%. The non-video workloads differ by at most 1% in that pair.

Raw artifacts, relative to the repository:

- `target/benchmark-results/media-pipeline-before-20260915/suite.json`
- `target/benchmark-results/media-pipeline-native-selection-20260915/suite.json`
- `target/benchmark-results/media-pipeline-native-selection-20260915/comparison.json`
- `target/benchmark-results/media-pipeline-native-selection-20260915/candidate.patch`
- `target/benchmark-results/media-pipeline-native-selection-repeat-20260915/suite.json`
- `target/benchmark-results/media-pipeline-native-selection-repeat-20260915/comparison.json`
- `target/benchmark-results/media-pipeline-baseline-repeat-20260915/suite.json`
- `target/benchmark-results/media-pipeline-native-selection-repeat-20260915/repeated-pair-comparison.json`

Commands:

```bash
uv run --no-project python scripts/benchmark.py run --suite canonical --output target/benchmark-results/media-pipeline-before-20260915
# Apply the candidate, then leave sources unchanged for the whole suite.
uv run --no-project python scripts/benchmark.py run --suite canonical --output target/benchmark-results/media-pipeline-native-selection-20260915
uv run --no-project python scripts/benchmark.py compare target/benchmark-results/media-pipeline-before-20260915/suite.json target/benchmark-results/media-pipeline-native-selection-20260915/suite.json --json
```

## Verification and remaining experiments

The initial 16 focused media tests passed. These include sparse and backward
requests at multiple cache budgets, zero-cache sequential holds, VFR timestamps,
and exact RGBA comparison with an independent FFmpeg decode of H.264 footage
with B-frames and a 24-frame GOP. Review requested additional tests for final
frame holds, sparse VFR boundary requests, and cache hits ahead of the cursor;
those have been added and passed in the broader repository gate.

`DISPLAY=:0 VESTRA_WGPU_BACKEND=gl WGPU_BACKEND=gl ./scripts/check.sh`
passed, including formatting, workspace build and Clippy, workspace tests,
Python schema validation, and generated-schema freshness. The full log is
`target/benchmark-results/media-pipeline-check.log`.

The clean baseline checkout is `/tmp/vestra-media-baseline-20260915`, detached
at the baseline revision. Its completed second suite is under
`target/benchmark-results/media-pipeline-baseline-repeat-20260915` in the primary
worktree. Both candidate and baseline executables have been copied and hashed
alongside their repeat runs. A resource measurement script under
`target/benchmark-results/media-pipeline-resources-20260915/measure.sh` uses those
executables on the video-heavy and production workloads without compilation.
Its process-resource scope includes fixture generation and FFmpeg subprocesses;
the inner benchmark records retain their narrower render timing scope.

Both resource runs completed successfully and their raw benchmark records pass
the existing compatibility comparison. Peak RSS was 870,628 KiB baseline versus
862,116 KiB candidate for video-heavy, and 933,928 KiB versus 823,480 KiB for
production-edit. This single pair shows no memory increase on these workloads;
it is not a universal memory bound. The saved executable hashes and per-workload
`*.resources.txt`, JSON records, and comparison JSON are under the resource-run
directory above.

## Rejected coordinator refill candidate

The second candidate refills a render slot before writing the previous completed
frame to the encoder. The original loop polls a frame, writes it, then refills on
the next iteration. A regression test demonstrated that the first encoder write
had only two pending frames with a capacity of three. After reordering, it has
three. All 36 staged-pipeline tests pass, including cancellation, encoder errors,
out-of-order frames, final-drain failures, and publication. The test log is
`target/benchmark-results/media-pipeline-refill-tests.log`, and the observed
failure before the change is in `media-pipeline-refill-red.log` beside it.

The candidate adds no threads or frame copies. Encoder writes remain
synchronous and ordered. The first attempted suite under
`target/benchmark-results/media-pipeline-refill-20260915` is invalid: sharing the
Cargo target directory with the baseline checkout reused its decoder library in
the coordinator executable. A probe linked to that library returned one seek on
a zero-cache hold, proving that it was not the candidate decoder. Its manifest
is retained as `invalid-suite.json`, with the reason in `INVALID.md`.

The affected crates were rebuilt, and the linked-artifact probe now reports
zero seeks and two decodes, as required by the candidate's hold behavior.
Cargo's JSON build record confirms fresh builds of media, engine, and benchmark
from the primary worktree. Subsequent builds from the baseline checkout must
use a separate target directory. The separately saved native-selection and
baseline executable comparisons above are unaffected. The full repository gate
recorded above predates this coordinator candidate.

Review added short-run and single-slot cases, a mock that returns no completion
for a nonblocking poll, and submission failure during refill while an earlier
frame is ready. Allowing refill whenever the next frame was present proved
unsafe: repeated partial drains could grow the ready queue. A capacity-four
regression with completion order `3,4,5,6,0,7,8,1,9,10,11,2` reproduced the
overflow. Exceptional admission now requires exactly capacity ready frames,
including the next writable frame. All 38 staged tests pass; logs are
`media-pipeline-refill-bound-red.log` and `media-pipeline-refill-bound-tests.log`
under benchmark results. Review confirmed that ready frames remain bounded by
`2 * capacity - 1`, while combined ready/backend payloads can transiently reach
`2 * capacity` before the synchronous write. This is one more retained frame
than the previous combined bound.

The correctly linked, but pre-bound-fix, canonical run in
`media-pipeline-refill-corrected-20260915` was slower than native selection alone:
single video +27.0%, mixed dynamic +46.7%, video-heavy +18.1%, production +1.5%.
Its comparison is retained beside the raw results. The bounded version in
`media-pipeline-refill-bounded-20260915` also failed the retention criterion:
single video +10.4%, mixed dynamic +14.7%, video-heavy +8.8%, production +1.5%.
The coordinator and its candidate-specific tests were removed. The exact
experiment remains in `rejected-candidate.patch` beside the bounded run's
results and comparison. These runs do not isolate why scheduling changed the
timings; they are sufficient to reject this implementation on this workload.

## Hardware verification

Hardware preflight on this host initially needed `DISPLAY=:0` and the explicit
`/usr/lib/wsl/lib/nvidia-smi` path. GL adapter discovery reports
`D3D12 (NVIDIA GeForce GTX 1650 SUPER)`, classified as a discrete GPU, through
Mesa 26.0.8. Vulkan reports CPU llvmpipe. Discovery is not execution validation.
`DISPLAY=:0 VESTRA_WGPU_BACKEND=gl WGPU_BACKEND=gl
./scripts/verify-wgpu.sh --hardware` passed on the native-selection change after
removing the coordinator candidate. This includes general workspace tests,
strict renderer parity tests, and the strict CLI encoded-frame comparison.
Execution records confirm the NVIDIA adapter and GL backend. The log is
`target/benchmark-results/media-pipeline-native-hardware-check-20260915.log`.
Subsequent hardware performance comparisons are recorded below.

## CPU source cursor candidate

CPU sessions previously shared one cursor per asset in each render worker.
Separate clips can request different times from that asset in one output frame,
making the shared cursor seek backwards repeatedly. The first candidate keyed lazy
sessions by the existing compiled video `source_index`. Each source retains its
cursor across frames and prepared operations. The existing per-worker video
cache budget is divided by compiled video-slot count instead of asset count.
Decoder contexts and native frame storage can still grow with session count;
they are outside the RGBA cache budget and must be measured.

A repeated-frame regression with two same-asset clips at different offsets
reported one unnecessary seek before the change, zero afterwards, and exactly
two persistent sessions. Existing pixel assertions remain. All 139 CPU tests
pass (nine manual benchmarks ignored). Logs are
`media-pipeline-source-cursors-red.log` and `media-pipeline-source-cursors-tests.log`
under benchmark results. The first canonical run in
`media-pipeline-source-cursors-20260915` reduced video-heavy median time from
4,867 to 4,439 ms and eliminated 82 seeks. It retained sessions for completed
clips, however, so it is superseded by a bounded-retention variant.

The next variant released sessions untouched by its just-completed frame. After
composition, native contexts cover that worker's latest frame; during
composition they cover the union of its previous and current frame's sources.
Retired-session counters remain in cumulative metrics, and open count includes
reopened sessions. This prevents historical clips from accumulating contexts.
Review confirmed the lifecycle and counter accounting. The regression failed
with two retained contexts instead of one before this change; all 139 CPU tests
pass afterwards. Retention logs are
`media-pipeline-source-cursors-retention-red.log` and
`media-pipeline-source-cursors-retention-tests.log`.

The bounded canonical run, `media-pipeline-source-cursors-retention-20260916`,
records video-heavy at 4,393 ms (−9.7% versus native selection), with zero seeks.
Production-edit is 9,298 versus 9,093 ms (+2.3%); several unchanged scenarios
also rose slightly. A fresh paired timing/RSS comparison is required before
attributing this to the candidate. The fresh pair under
`media-pipeline-cursor-resources-20260916` showed video-heavy −11.1% and production
+0.4%. Peak RSS changed from 822,804 to 956,424 KiB for video-heavy and from
857,508 to 720,712 KiB for production.

A 24-clip, single-file workload then rejected this retirement policy: its median
rose from 4,184 to 5,018 ms (+19.9%), opening 192 decoders instead of eight.
Both release executables produced identical RGBA frame checksums for all 360
frames of the 1280×720, 12-second output. The fixture, executables, five-sample
reports, resource records, and reproduction script are under
`media-pipeline-serial-clips-20260916`. Neither per-source variant is retained.

The current pool candidate preserves one cached primary decoder per asset and
worker. Exact same-asset/time reads share a cursor within a frame; other times
take the next unused cursor, opening an extra only when necessary. Extra
cursors have zero optional cache allowance and are released when unused by the
latest frame. The primary remains available across serial clips and idle frames.
This preserves the original total cache allowance while separating simultaneous
timelines. Draw-order changes may require seeks when cursors change trajectories.

All 140 CPU tests pass, including reordered-source pixels against a fresh
backend, identical-time reuse, serial/idle reuse, cumulative retirement counters,
and the total cache allowance. A decoder-open error previously bypassed the
worker's error state and could silently omit video; a failing regression
reproduced this and the candidate now propagates that error. The prepared SDK
documentation clarifies that source video remains external to its snapshot.
The final pool's serial workload comparison under
`media-pipeline-cursor-pool-serial-20260916` records medians of 4,513 ms for native
selection alone and 4,493 ms for the pool (−0.44%, effectively unchanged), with
eight decoder opens in both. All 360 decoded RGBA frame checksums match. A
separate simultaneous-offset render under
`media-pipeline-cursor-pool-concurrent-20260916` also matches every decoded frame:
90 frames, 1280×720, three seconds.

The full canonical pair (`media-pipeline-cursor-pool-20260916` and
`media-pipeline-native-selection-pool-pair-20260916`) had unchanged controls move
by 2–18%, so its timing differences are inconclusive. The follow-up experiment
under `media-pipeline-cursor-pool-alternating-20260916` alternates saved release
executables in native/pool/pool/native order three times, after one discarded
invocation per executable. Each invocation records one canonical sample;
there are six samples per implementation and workload.

| Workload | Native-selection median, ms | Pool median, ms | Change |
| --- | ---: | ---: | ---: |
| Video-heavy | 6,351 | 5,397.5 | −15.0% |
| Production edit | 10,387.5 | 10,412.5 | +0.24% |

Video-heavy consistently drops from 4,144 native decodes and 82 seeks to 2,451
decodes and zero seeks. Decoder opens increase from 16 to 24. Peak process RSS
ranges increase from 529,752–540,424 KiB to 618,692–629,412 KiB, about 90 MiB.
Production keeps 16 opens and eight seeks; its RSS ranges overlap. The resource
scope includes fixture creation and subprocesses. Timing variance remains
substantial, and these results support a targeted video-heavy benefit, not a
universal speedup. Raw samples, executable hashes, resource records, and the
reproduction script are retained in the alternating-run directory.

The final pool passed `./scripts/check.sh` with the GL environment above; the log
is `media-pipeline-cursor-pool-check-20260916.log` under benchmark results. Four
video preparation/timing/transition/crop tests also passed with both
`VESTRA_REQUIRE_WGPU=1` and `VESTRA_REQUIRE_HARDWARE_WGPU=1`; execution records in
`media-pipeline-cursor-pool-hardware-video-20260916.log` confirm the NVIDIA GL
adapter. These parity tests use renderer fixtures; the real-media checksum
comparisons above separately verify encoded CPU output. The pool is retained
for the measured video-heavy gain, with its explicit memory tradeoff.

## Coverage and current status

| Requested area | Current evidence and next action |
| --- | --- |
| Decoder/session reuse | The CPU pool separates simultaneous asset/time trajectories, eliminates 82 video-heavy seeks, and preserves serial reuse. WGPU's per-asset sessions are exercised by the hardware canonical and concurrent-offset measurements below. |
| Sequential decode | Native selection has repeated CPU timing, peak-RSS evidence, and strict hardware correctness validation; the access matrix measures sequential requests and advancing holds. |
| GOP/keyframe-aware seeking | Real long-GOP/B-frame pixels and boundary requests verify backward keyframe seeking. The access matrix measures sparse forward and scrub costs; no unsupported forward-seek threshold is introduced. |
| Decoded-frame caching | Explicit coverage intervals and current-frame holds tested; the access matrix measures zero/one/four-frame/64-MiB budgets and repeated scrubbing. |
| Decode-ahead/prefetch | A bounded additional native frame improves measured GPU wall time. CPU always-on prefetch is rejected; the WGPU-only policy is retained after output checks and the repository gate. |
| Avoiding invisible decode work | GPU-plan culling removes unused hidden video reads while preserving matte/mask dependencies; real hardware timing and exact decoded-output evidence are below. |
| Independent video-source parallelism | CPU workers decode concurrently; the one/two/four/eight-worker sweep below measures throughput, repeated decoding, sessions, and memory. |
| Pixel formats/conversions | Native selection reuses scaler output and converts only requested frames. Real YUV420P H.264/B-frame output is byte-checked against FFmpeg; timing evidence is above. |
| Decode/render/encode overlap | CPU worker and GPU buffering sweeps measure the existing staged pipeline. Native prefetch overlaps decoding with composition; scoped encoder writes overlap one WGPU submission and reduce measured wall time. |
| Bounded stage queues | CPU command/completion and ready-frame counts are bounded; prefetch bounds pending native frames at two and defers errors in order. Async feeding joins every write before another write and tests the combined payload bound, including the writer. |
| Multiple frames in flight | Hardware GL depths one/two/three measured, with an alternating two/three repeat and exact output checks; CPU worker-count sweep completed. Defaults retained. |
| Asynchronous encoder feeding | WGPU feeding overlaps a blocking FFmpeg pipe write with one submission. CPU feeding remains synchronous after near-neutral measurements. Ordered acknowledgements preserve failure/cancellation/publication contracts. |
| Audio buffers and analysis cache | Release measurements confirm shared FFT/decode work and prepared reuse. Scratch reuse is retained after real-WAV gains and unchanged render controls. |
| Thread/resource allocation | Worker-count sweep retains eight on this host despite repeated native decoding; lower counts save memory but substantially slow both canonical workloads. Decoder/encoder thread settings are not independently isolated. |
| Stalls and idle CPU/GPU time | Stage timings, buffering counters, prefetch and async-feeding measurements assess overlap. Wall-time gains do not establish GPU occupancy or eliminate stalls in every driver call. |

Native frame selection, the CPU cursor pool, audio FFT scratch reuse, GPU
video culling, WGPU native prefetch, and asynchronous encoder feeding are
retained on measured gains and correctness verification. CPU asynchronous
feeding was rejected after near-neutral measurements.

## Audio analysis measurements

At revision `78fd8ae`, the existing release-build audio benchmarks were run
three times from a saved test executable without concurrent compilation or
other benchmarks. Records and executable hash are under
`target/benchmark-results/media-pipeline-audio-20260916`. The synthetic matrix
streams stereo silence and measures analysis, not FFmpeg decoding. Medians:

| Duration | Frequency bands | Analysis time, ms | FFT calls |
| --- | ---: | ---: | ---: |
| 60 seconds | 1 | 310 | 11,992 |
| 60 seconds | 10 | 304 | 11,992 |
| 60 seconds | 50 | 307 | 11,992 |
| 600 seconds | 1 | 3,064 | 119,992 |
| 3,600 seconds | 1 | 18,436 | 719,992 |

The real-WAV transformed-signal benchmark analyzes 60 seconds and applies 1,
10, or 50 distinct transform pipelines. Every run records one master decode,
one raw feature series, and 12,002 FFT calls at every fan-out. Transform
preparation takes 0, 1, and 5 ms respectively at the timer's millisecond
resolution. Analysis ranges from 372 to 478 ms. Whole-process peak RSS is
14,784–15,148 KiB for the synthetic matrix and 58,076–58,436 KiB for the WAV
benchmark, including fixture creation and FFmpeg subprocesses.

The repository gate also executes
`prepared_audio_analysis_is_reused_across_random_access_and_video_operations`:
real audio analysis runs once through random-access frames, a mock-sink video
operation, and another frame. This verifies prepared analysis reuse, not real
encoder throughput. PCM byte/sample vectors and spectral input/output buffers
already retain allocations between chunks.

Inspection of the installed RustFFT implementation found that `Fft::process`
allocates scratch storage per call. The scratch-reuse candidate supplies one
analyzer-owned scratch vector to `process_with_scratch` for both channels.
All 24 audio-analysis correctness tests pass in release mode, with three manual
benchmarks ignored in that check. Three candidate synthetic runs have medians
of 298/302/300 ms for the 60-second 1/10/50-band cases, 2,977 ms for ten minutes,
and 17,937 ms for one hour. The latter is 2.7% below the baseline median.
Records are under `media-pipeline-audio-scratch-20260916`.

An additional real-WAV experiment alternates saved baseline/candidate binaries
in baseline/candidate/candidate/baseline order three times. Six samples per
implementation give baseline analysis medians of 380/381/384 ms and candidate
medians of 371.5/369/368.5 ms for 1/10/50 transforms: reductions of 2.2%, 3.1%,
and 4.0%. Decode and FFT counts remain identical. Logs and process-resource
records are under `media-pipeline-audio-scratch-alternating-20260916`.
These are analysis-stage gains, not end-to-end render speedups.

The render comparison uses the saved CPU-pool executable as baseline and a
fresh scratch-reuse executable, with baseline/candidate/candidate/baseline
invocations, each containing one warmup and three samples. The completed
video-heavy control has pooled medians of 4,711.5 and 4,679 ms (−0.7%), with
substantial within-run variation. This workload performs no audio analysis;
production-edit has output audio but no analysis-driven visuals. Neither can
establish an FFT speedup. They check for broader render regressions, while the
real-WAV measurements exercise the changed path. Records and reproduction
script are under `media-pipeline-audio-scratch-render-20260916`.
Production-edit completed with pooled medians of 9,664.5 ms baseline and
9,640.5 ms candidate (−0.25%, effectively unchanged). Review of the installed
RustFFT 6.4.1 implementation confirms that `process` delegates to
`process_with_scratch` with the same plan-sized allocation. Sequential channel
calls can share the workspace without clearing it; the candidate retains one
scratch vector for the analyzer's lifetime. No transform order or normalization
changes. The broader repository gate passed, including formatting, Clippy,
workspace tests and schema checks; the log is
`media-pipeline-audio-scratch-check-20260916.log` under benchmark results.
Scratch reuse is retained for the measured analysis-stage improvement.

## WGPU visibility candidate

The previous WGPU submission called `upload_video_layers` before constructing the GPU
frame plan. That traversal includes hidden layers and their masks, whereas
`append_layer` in the frame planner omits hidden presentation and explicitly
adds matte sources needed by visible consumers. A candidate should derive
required video reads from actual raster operations, preserving hidden matte
dependencies and owned video masks. Simply filtering uploads on `visible`
would omit valid matte inputs.

A strict NVIDIA GL counter test reproduced an extra frame request for hidden
video with no consumer. The candidate walks validated `RenderRasterLayer`
operations instead. Upload errors after readback acquisition use the existing
abort path to release slots and static-cache reservations. Review also found
that a visible video reused as a matte appears in multiple operations; a
regression reproduced duplicate requests. A per-frame set now requests each
compiled video slot once, preserving distinct slots for different timelines.

Four strict hardware video tests pass, including pixels and counters for an
unused hidden video, a required hidden matte, an owned video mask, a hidden
mask owner, and shared visible/matte input. Logs are
`media-pipeline-culling-red-20260917.log`,
`media-pipeline-culling-duplicate-red-20260917.log`, and
`media-pipeline-culling-final-video-tests-20260917.log` under benchmark results.
The broader repository gate passed; its log is
`media-pipeline-culling-check-20260917.log` under benchmark results.

The real-media visibility workload is a three-second 1280×720 timeline with two
same-asset clips at different offsets, one hidden. Both saved release CLIs use
hardware NVIDIA GL, verified in every report. Six alternating samples per
implementation give medians of 3,392.5 ms baseline and 1,774.5 ms candidate
(−47.7%). Requests fall from 181 to 91, native decodes from 788 to 90, and seeks
from 19 to zero in every sample. Peak process RSS ranges are 464,620–466,452 KiB
and 413,292–415,560 KiB. The saved baseline predates audio scratch reuse, but this
workload has no audio or analysis and the audio change is outside its path.

Both initial and final timed outputs match exactly after FFmpeg RGBA decoding:
90 frames, 1280×720, three seconds. The fixture, saved executable hashes, raw
reports, process resource records, and checksums are under
`media-pipeline-culling-render-20260917`. These results establish a gain for
hidden source avoidance; unchanged canonical video-heavy and production GPU
controls completed under `media-pipeline-culling-gpu-controls-20260917`.
The same script subsequently measures candidate pipeline depths one and two,
against the depth-three control samples, with explicit adapter checks on every
sample. Review confirmed that compiled video slot identities remain unique
across nested groups, masks, and distinct timelines.

The controls use the audio-scratch executable as baseline, so culling is the
only runtime change. Six samples per variant (two three-sample invocations in
baseline/candidate/candidate/baseline order) give canonical wall medians:

| Hardware GL workload | Baseline, ms | Culling, ms | Change |
| --- | ---: | ---: | ---: |
| Video-heavy | 6,871 | 6,932.5 | +0.9% |
| Production edit | 5,790.5 | 5,759.5 | −0.5% |

Decode counts are unchanged: 2,313 and 453 respectively. These small timing
differences do not establish a control-workload speedup or regression. The
culling change is retained for its large measured hidden-video gain, exact
decoded output, dependency tests, and passing repository gate. The benchmark
console labels the adapter `other` from its raw WGPU device type; the explicit
NVIDIA GL identity in every sample and accelerated GL preflight establish the
hardware class independently. This is not a software-Vulkan measurement.

## Hardware buffering sweep

Using the same culling executable and NVIDIA GL adapter, the first sweep gives
the following wall medians. Depths one and two have three samples after one
warmup each; depth three uses the six control samples above.

| Workload | Depth one, ms | Depth two, ms | Depth three, ms |
| --- | ---: | ---: | ---: |
| Video-heavy | 7,210 | 6,627 | 6,932.5 |
| Production edit | 6,847 | 5,246 | 5,759.5 |

Actual peak frames in flight match the requested depths. Video-heavy reports
40,552,960 / 47,928,320 / 55,303,680 bytes of staging memory; production reports
48,713,464 / 56,094,200 / 63,474,936 bytes. Each additional slot costs roughly
7 MiB here. Blocking-poll and slot-wait counters are zero on this GL path at
all depths; those counters do not establish GPU occupancy or absence of stalls
inside other driver calls. Production's depth-two sample reports about 2.16 s
in encoder writes, which motivates measuring asynchronous feeding separately.

The first sweep was not interleaved. An alternating 2/3/3/2 repeat, with one
warmup and three samples per invocation, completed in
`media-pipeline-depth-alternating-20260917`. Its six-sample medians are:

| Workload | Depth two, ms | Depth three, ms | Two versus three |
| --- | ---: | ---: | ---: |
| Video-heavy | 6,856 | 6,841.5 | +0.2% |
| Production edit | 5,523.5 | 6,076.5 | −9.1% |

The video-heavy timing advantage did not repeat; production's did. Depth two
is a useful measured setting for this NVIDIA-through-GL path and production
workload, with lower staging memory in both cases. No cross-backend default is
changed on this single-adapter evidence; `VESTRA_WGPU_IN_FLIGHT=2` remains the
existing explicit control.

The same release CLI also rendered two simultaneous offsets of real video at
depths one, two, and three. All 90 decoded RGBA frame checksums match across
depths, with 1280×720 dimensions and three-second duration. Every report selects
the NVIDIA GL adapter. Executable hash, reports, videos, checksums, and the
verification script are in `media-pipeline-depth-pixels-20260917`.

## CPU worker allocation sweep

An executable-only experiment overrides the CPU worker count to one, two, four,
or eight. Its exact instrumentation patch and executable hash are saved under
`target/benchmark-results/media-pipeline-cpu-workers-20260917`; the production
source was restored before measurement. Each case uses the canonical workload
at 1280×720, one warmup, and three measured renders. All reports select CPU,
and the actual pipeline depth matches the requested worker count.

| Workload | Workers | Wall median, ms | Native decodes | Live video sessions | Peak process RSS, KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Video-heavy | 8 | 4,760 | 2,451 | 24 | 1,009,864 |
| Video-heavy | 4 | 6,873 | 1,248 | 12 | 702,268 |
| Video-heavy | 2 | 12,165 | 630 | 6 | 556,444 |
| Video-heavy | 1 | 22,787 | 315 | 3 | 483,820 |
| Production edit | 8 | 10,031 | 3,555 | 16 | 976,468 |
| Production edit | 4 | 15,081 | 1,800 | 8 | 622,196 |
| Production edit | 2 | 27,628 | 906 | 4 | 464,464 |
| Production edit | 1 | 48,968 | 453 | 2 | 364,928 |

The original process stopped before the final production/one-worker case
produced a report. After confirming that neither its session nor its process
remained, only that case was rerun with the same saved executable and settings
in `media-pipeline-cpu-workers-resume-20260917`. Its three wall samples were
48,839 / 48,968 / 49,086 ms. The interrupted artifacts remain intact. The resumed
case was measured in a separate host session; the fixed-order sweep is evidence
of the large resource/throughput tradeoff, not a precise scaling curve.

Fewer workers reduce repeated native decoding, decoder contexts, scratch
storage, and process memory, but substantially increase render wall time on
both workloads. Video-heavy has no seeks at any count; production has one seek
per worker. The current automatic cap of eight is retained on this host. RSS
includes fixture generation and FFmpeg subprocesses and is not solely renderer
memory. This sweep does not establish an optimal decoder or encoder thread
count independently of render-worker allocation.

## Native decoder access and cache budgets

The ignored release test `video::benchmarks::video_access_benchmark_matrix`
measures native-decoder requests separately from opening the stream. It creates
six seconds of 1280×720, 30 fps, H.264 YUV420P footage with a 60-frame GOP and
three B-frames. Every pattern/budget pair first compares every requested image
byte-for-byte with an independent FFmpeg RGBA decode. Three subsequent samples
measure requests without pixel-comparison work. Each pass opens a fresh decoder.

```bash
VESTRA_VIDEO_ACCESS_BENCH_OUTPUT=target/benchmark-results/video-access-new/results.json \
  cargo test -p vestra-media --release video_access_benchmark_matrix -- --ignored --nocapture
```

The completed run uses a saved release executable under
`target/benchmark-results/media-pipeline-video-access-20260917`, with its build
record, hash, FFmpeg version, fixture hash, and `run-1/results.json`. It passed
all pixel comparisons and all 64 passes. The following are median accumulated
request times; decoder opening and fixture generation are excluded.

| Pattern | Cache budget | Requests | Native decodes | Seeks | Cache hits | Request time, ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Sequential | 0 | 180 | 180 | 0 | 0 | 488.9 |
| Sequential | 64 MiB | 180 | 180 | 0 | 0 | 511.1 |
| Advancing holds | 0 | 180 | 61 | 0 | 0 | 175.7 |
| Advancing holds | 64 MiB | 180 | 61 | 0 | 120 | 197.5 |
| Sparse forward | 0 | 9 | 180 | 0 | 0 | 358.2 |
| Sparse forward | 64 MiB | 9 | 180 | 0 | 0 | 356.8 |
| Repeated scrub | 0 | 48 | 2,436 | 19 | 0 | 4,722.6 |
| Repeated scrub | 1 frame | 48 | 2,436 | 19 | 0 | 4,759.1 |
| Repeated scrub | 4 frames | 48 | 2,436 | 19 | 0 | 4,767.9 |
| Repeated scrub | 64 MiB | 48 | 609 | 4 | 36 | 1,197.8 |

Holds advance within each presentation interval, confirming that a disabled
optional cache does not force repeated decoding or seeking. Sparse forward
requests still decode intervening native frames but convert only requested
images. The scrub sequence includes requests immediately before and after GOP
boundaries. A 64 MiB cache holds its twelve requested images and avoids work on
the three repeated traversals; one- and four-frame caches cannot hold that
working set. These measurements validate the cache benefit for repeated access
and the cost of misses, without supporting a universal cache-budget change.

The fixed-order, three-sample run is exploratory; small timing differences do
not establish significance. Process RSS includes a 663,552,000-byte independent
reference buffer and must not be interpreted as decoder/cache memory. This
benchmark does not compare the former eager-conversion cache or establish a
forward-seek threshold.

The full `./scripts/check.sh` gate passed with the GL environment described
above, including workspace Clippy, tests, and schema checks; its log is
`media-pipeline-video-access-20260917/check-fixed.log`. A package-only Clippy
attempt omitted renderer features and failed on unrelated unused/dead renderer
items. The first canonical attempt found a constant assertion in the new
benchmark's release-mode guard; that guard now returns a runtime test error
in debug builds, and the corrected canonical gate passes without relaxing
warnings.

A second release run after the guard correction passed in 82.76 seconds;
`run-2/results.json` reproduces every request/decode/seek/cache-hit count.
Repeated scrub medians were 4,846.6 ms without a cache and 1,218.8 ms with
64 MiB, confirming the large repeated-access benefit.

## Bounded native prefetch

The decoder actor can decode one additional native frame after replying to the
renderer. It uses its existing owner thread and retains at most two pending
native frames, rather than one. It does not predict timestamps or convert
speculative pixels. Already-queued demand takes priority. Prefetched errors stay
in decode order and surface only when requested; seeking clears queued frames
and errors. Demand-request/cache counters remain separate from native work.

The actor holds its metrics lock across reply delivery, prefetch, and counter
publication, so a subsequent statistics snapshot includes speculative work.
Consequently statistics collection and teardown can wait for an in-progress
prefetch. One native frame bounds storage/work count, not elapsed time: decoding
it may read multiple packets. A disconnect observed before starting prefetch
skips that work; shutdown racing with it may wait for that one operation.

The first alternating baseline/candidate/candidate/baseline experiment uses
one warmup and three samples per invocation on each canonical workload. Its
saved executables, exact prototype patch, raw reports, and process resource
records are in `target/benchmark-results/media-pipeline-prefetch-20260917`.
The baseline is the retained culling executable. Hardware reports explicitly
select NVIDIA GL, with a fresh accelerated-GL preflight; Vulkan remains llvmpipe.

| Workload | Baseline wall median, ms | Prefetch wall median, ms | Change | Native decodes, baseline → candidate |
| --- | ---: | ---: | ---: | --- |
| Hardware GL video-heavy | 6,079 | 5,853.5 | −3.7% | 2,313 → 2,374 |
| Hardware GL production | 5,081.5 | 4,497 | −11.5% | 453 → 456 |
| CPU video-heavy | 4,354.5 | 4,307 | −1.1% | 2,451 → 2,472–2,475 |

GPU peak RSS increased from 487,272–487,720 to 490,888–491,504 KiB for
video-heavy and from 490,312–490,352 to 492,136–492,436 KiB for production.
CPU video-heavy ranged from 1,048,636–1,061,832 to 1,063,468–1,096,000 KiB.
The additional native frame is outside the RGBA cache budget. Speculative work
increases actual decodes while overlapping them with composition; aggregate
decode time therefore need not fall when wall time improves.

The process stopped during the final CPU production baseline invocation.
Completed pre-interruption baseline samples were 9,101 / 9,138 / 9,108 ms,
versus candidate samples 9,024 / 9,054 / 9,092 and 9,041 / 9,054 / 10,158 ms.
After confirming that the process was gone, only the missing case was rerun
under `media-pipeline-prefetch-resume-20260917`. Its 11,230 / 11,354 / 11,246 ms
samples show a host-session timing shift; pooling them with the earlier
candidate would exaggerate any gain. The uninterrupted CPU production repeat
below resolves the retention decision.

Twenty-one native video tests and 68 engine tests passed with prefetch enabled.
Coverage includes queue bounds, zero-cache holds, EOF, deferred errors cleared
by seek, and real H.264/VFR pixels. The final candidate removes the temporary
experiment switch. Its release CLI rendered two simultaneous video offsets on
CPU and hardware GL; each backend's 90 decoded RGBA frames match its baseline
exactly, with 1280×720 dimensions and three-second duration. Reports, checksums,
and executable hashes are under the candidate's `pixels` directory. The
always-on candidate passed the repository gate (`check.log`); the subsequent
CPU repeat and final WGPU-only policy below supersede that candidate.

The uninterrupted CPU production repeat completed under
`media-pipeline-prefetch-resume-20260917/repeat`: baseline median 11,546 ms,
always-prefetch median 11,815.5 ms (+2.3%). Always-on CPU prefetch is rejected.
The final policy enables prefetch only for WGPU sessions through an optional,
default-no-op decoder-session hint. Ordinary native sessions keep prefetch
disabled; existing custom decoder implementations remain compatible. A real
native-session test verifies unchanged pixels/request counts and an additional
native decode only after enabling the hint, including the metrics barrier.

The WGPU-only CPU control is in the sibling `gpu-only-repeat` directory. Opening
baseline samples were 11,043 / 11,092 / 11,069 ms; closing baseline samples were
11,916 / 11,778 / 11,769 ms. Candidate samples were 11,083 / 11,135 / 11,146 and
11,231 / 11,303 / 11,661 ms. The drift between controls prevents attributing the
pooled medians (11,430.5 versus 11,188.5 ms) to a CPU speedup. It does not reproduce
the always-on slowdown. CPU sessions perform no speculative work, as verified
by the native-session test.

The WGPU-only policy passed `./scripts/check.sh` with the GL environment above;
the full log is `media-pipeline-prefetch-20260917/check-gpu-only.log`. Its final
release CLI again produced identical baseline/candidate RGBA checksums for all
90 frames on both CPU and NVIDIA GL. CPU native decodes were 1,634 in both
variants; GPU counts were 788 versus 808, with 19 seeks in both. Final reports,
output properties, executable hashes, and checksums are in `pixels-gpu-only`.
The WGPU-only policy is retained for its measured GPU gains, bounded additional
storage, and verified output/lifecycle behavior. No CPU speedup is claimed.

## Scoped asynchronous encoder feeding

Dynamic WGPU renders now write the next ordered frame on a scoped writer thread
while the calling thread evaluates and submits at most one further frame into a
free backend slot. The writer borrows the existing sink and completed frame;
there is no pixel copy or persistent encoder queue. Every write is joined before
another write, progress delivery, cancellation cleanup, finalization, or output
publication. Static renders and CPU feeding keep their previous paths.

A successful write is acknowledged before a speculative submission error is
reported. A write error wins if both operations fail; cancellation from its
progress callback wins over the speculative error. Writer panic and spawn
failure use structured encoder failure cleanup. Tests exercise concurrent
submission, caller-thread progress, cancellation, both error orders, panic,
short runs, and repeated partial drains with out-of-order completions. The
transient combined payload bound is `2 * backend capacity`, including the
writer's borrowed frame; the ready queue stays below that bound. This adds at
most one payload to the previous combined bound, without an unbounded queue.

The release experiment used the retained WGPU-prefetch executable as baseline.
It alternated baseline/candidate/candidate/baseline invocations, each with one
warmup and three samples at 1280×720. The hardware cases used depth three and
explicitly selected NVIDIA GL, confirmed by fresh accelerated-GL preflight on
2026-09-18. Saved binaries, hashes, prototype patch, scripts, reports, and
resource records are under `target/benchmark-results/media-pipeline-async-20260917`.

| Workload | Baseline wall median, ms | Async wall median, ms | Change |
| --- | ---: | ---: | ---: |
| Hardware GL video-heavy | 6,130 | 5,846.5 | −4.6% |
| Hardware GL production | 4,877.5 | 4,080.5 | −16.3% |
| CPU video-heavy | 4,599.5 | 4,551.5 | −1.0% |
| CPU production | 9,638.5 | 9,531 | −1.1% |

CPU changes are small, and video-heavy's baseline drifted downward during the
run. CPU feeding is therefore left unchanged. The final implementation removes
the experiment switch and enables overlap only for WGPU. Scoped thread creation
and joining are included in wall time. Actual writer duration is recorded
separately from composition, and those durations overlap: they must not be
summed to infer elapsed time. For example, production's median writer time
rises from 2,137 to 2,451 ms while total wall time falls. Native decode counts
stay at 2,374 for GPU video-heavy and 456 for GPU production in both variants.

Peak process RSS on GPU video-heavy ranges from 493,140–493,800 KiB baseline to
493,792–493,824 KiB candidate; production ranges from 493,132–493,192 to
494,388–494,480 KiB. These process measurements include fixture generation and
FFmpeg subprocesses, not just frame storage. They do not replace the tested
payload-count bound.

Non-video GL controls use the same alternating procedure. Dynamic animation
medians are 1,551 versus 1,494.5 ms, and layered effects are 2,007 versus 2,003 ms.
The unchanged static-mask path measures 486.5 versus 483 ms. These controls show
no observed regression. The particle control could not run on the saved baseline:
its 4 MiB additive upload buffer exceeds that GL device's configured 3,686,400-byte
maximum. The CLI diagnostic is preserved in `controls/particle-diagnostic.json`;
it is a pre-existing hardware-path limitation, not a successful particle check.
Completed control records are in `controls` and `controls-supported`.

The final WGPU-only policy passes all 64 selected engine tests without an
experiment environment variable (`final-engine-tests.log`). Its release CLI and
the baseline each rendered two simultaneous offsets of real video on CPU and
NVIDIA GL. All 90 decoded RGBA frames match exactly within each backend, with
1280×720 dimensions and three-second duration. Reports, output properties,
checksums, saved CLIs, and hashes are in `pixels-final`.

`DISPLAY=:0 VESTRA_WGPU_BACKEND=gl WGPU_BACKEND=gl ./scripts/check.sh` passed
on the final source, including formatting, workspace build and Clippy, workspace
tests, Python schema validation, and schema freshness. Its complete log is
`check-final.log`. Final read-only review found no correctness blocker. Async
feeding is retained for the measured WGPU gains; CPU feeding stays unchanged.
These results apply to this NVIDIA-through-GL environment and measured workloads,
not native Windows D3D12/Vulkan performance or a universal GPU-utilization claim.
