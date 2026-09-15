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

| Requested area | Current evidence and next action |
| --- | --- |
| Decoder/session reuse | Sessions persist per render worker but are keyed by asset. The video-heavy fixture has three clips using two assets at different offsets, producing 82 seeks. Both CPU and WGPU already receive a stable `source_index`; measure separate cursors for these trajectories with a divided cache budget. |
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
| Audio buffers and analysis cache | PCM parser reuses buffers and prepared projects retain analysis; execute reuse tests and isolated measurements. |
| Thread/resource allocation | Compare decode/render/encode allocation on video-heavy and production workloads. |
| Stalls and idle CPU/GPU time | Use stage timings and hardware measurements to assess overlap candidates. |

Native frame selection is retained on repeated CPU gains and correctness
verification. Hardware performance and the remaining experiment matrix are
unfinished; this report does not claim completion of Subblock 2.
