# Media pipeline and concurrency

This report tracks Optimization Subblock 2. Work is in progress; the first
candidate below has two canonical candidate measurements, and the remaining
experiments have not yet been completed.

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
Hardware performance comparisons remain to be run.

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

| Requested area | Current evidence and next action |
| --- | --- |
| Decoder/session reuse | The CPU pool separates simultaneous asset/time trajectories, eliminates 82 video-heavy seeks, and preserves serial reuse; measurements and memory cost are above. WGPU still uses one cursor per asset and needs measurement. |
| Sequential decode | Native-selection change above has repeated CPU timing, peak-RSS evidence, and strict hardware correctness validation. Hardware timing remains. |
| GOP/keyframe-aware seeking | Existing backward keyframe seek has a real long-GOP pixel oracle; measure sparse forward access. |
| Decoded-frame caching | Explicit coverage intervals and current-frame holds tested; measure random-access tradeoffs. |
| Decode-ahead/prefetch | One native lookahead exists; evaluate bounded additional prefetch. |
| Avoiding invisible decode work | Candidate removes unselected-frame conversion; inspect and exercise visibility culling. |
| Independent video-source parallelism | CPU workers already decode concurrently; compare resource allocations and source sharing. |
| Pixel formats/conversions | Candidate reuses scaler output and delays conversion; evaluate direct RGBA handling. |
| Decode/render/encode overlap | Existing staged CPU workers overlap with FFmpeg; evaluate coordinator stalls. |
| Bounded stage queues | CPU command/completion and ready-frame counts are bounded; evaluate decoder/encoder queue changes. |
| Multiple frames in flight | Existing CPU workers and WGPU slots need workload-specific depth measurements. |
| Asynchronous encoder feeding | Current frame writes synchronously feed FFmpeg stdin; experiment without weakening failure/cancellation/publication contracts. |
| Audio buffers and analysis cache | Three release runs confirm shared FFT work and one decode across transformed-signal fan-out; prepared reuse test passes. FFT scratch-reuse candidate is under measurement, below. |
| Thread/resource allocation | Compare decode/render/encode allocation on video-heavy and production workloads. |
| Stalls and idle CPU/GPU time | Use stage timings and hardware measurements to assess overlap candidates. |

Native frame selection and the CPU cursor pool are retained on measured CPU
gains and correctness verification. Hardware performance and the remaining experiment matrix are
unfinished; this report does not claim completion of Subblock 2.

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
allocates scratch storage per call. An unretained candidate supplies one
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

## Next visibility experiment

Current WGPU submission calls `upload_video_layers` before constructing the GPU
frame plan. That traversal includes hidden layers and their masks, whereas
`append_layer` in the frame planner omits hidden presentation and explicitly
adds matte sources needed by visible consumers. A candidate should derive
required video reads from actual raster operations, preserving hidden matte
dependencies and owned video masks. Simply filtering uploads on `visible`
would omit valid matte inputs. This is a source-level finding; decode counters,
pixel parity, and hardware timing are still required before changing behavior.
