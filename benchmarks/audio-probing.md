# Native audio inspection results

These measurements support [#110](https://github.com/evgen2571/vestra/issues/110).
Metadata-first inspection removes whole-source decoding from ordinary preparation.
It does not establish exact duration or integrity, and rendering still pays for
the decoded-sample verification described in the [audio contract](../docs/reference/audio.md).

## Environment and protocol

- Linux x86_64, AMD Ryzen 5 3600 (6 cores / 12 threads), Rust 1.95.0.
- Baseline: #109's `9eb6801599bbb6b89c566866bf2b4e8060557035`.
  Candidate production code: `2c78378e980719da1730b407fe817b77662cc612`;
  this report and the benchmark-only harness are added separately.
- Both final executables link the same release FFmpeg libraries: native release
  8.1.3, source `a35157abb96167fd7308edc92b55408d5bb084b0`, identical
  configuration and library search path. Static libraries include avcodec,
  avformat, swresample and swscale; avfilter/programs are disabled. The external
  fixture generator/encoder/verifier is FFmpeg 8.1.2.
- Release builds, selected CPU renderer, warm OS cache, no concurrent compilation
  or other benchmark workers. Loading JSON and generating fixtures are outside
  operation timings. Validation includes media, output/tool and CPU readiness.
- Ordinary operations use one warmup and five measured calls in one loaded-project
  process. Preparation runs afresh each time. Prepared rendering prepares once
  outside its timed calls; its first call includes deferred verification.
- Full 30-minute encoding uses one warmup plus one measured call per reported
  comparison. The FLAC baseline's first measured call is selected from a retained
  five-sample run; its other samples remain in the raw data. Full moving-video
  prepared repeats were omitted after observing ~141 seconds per encode; repeated
  prepared AV execution is covered by the short matrix and full FLAC outputs.
  Single full-length samples establish correctness and coarse cost, not a precise
  throughput improvement.

The preserved final workflow executables have SHA-256 hashes
`cdfb34a02fb32b44d12da69dbbe8ad8561d081f02bcb7b58a0f8a8f8f0b7d94c`
(baseline) and
`ece6d8b5aca2167bfd59ebc3b5af3ebb9b4656e6930cc77b2c612ac3bd3fa9d3`
(candidate). Negative controls verify that baseline/strict preparation resolves
the misleading FLAC to 0.375 seconds, while default preparation reports 10 seconds
with `VESTRA-AUDIO-DURATION-REPORTED`.

## Workloads and reproduction

Use the [benchmark commands](README.md) and the ignored
`audio_workflow_benchmark` test. Fixtures are stereo 48 kHz, with short sources of
1.25 seconds and long sources of 1,800 seconds. Generate each format from the
same 440 Hz tone, changing the codec/extension to `pcm_s16le`/WAV,
`flac`/FLAC, `libmp3lame`/MP3 or `aac`/M4A:

```bash
ffmpeg -nostdin -v error -f lavfi \
  -i 'sine=frequency=440:sample_rate=48000:duration=1800' \
  -ac 2 -c:a flac long.flac
```

The eight-source workload uses distinct FLAC files with frequencies
`440 + 37 * index` Hz, for indices 0 through 7. Each clip has gain 1/8.
The normal matrix has a 64×64 preview-quality output at 24 fps, explicit duration
1.25 seconds, and audio clips at timeline/source zero with `trim_end: 1`.
Its final 0.25 seconds are authored silence. This measures a bounded excerpt of
each long source, not full-length encoding throughput.

The short AV source combines `testsrc2=size=64x64:rate=24:duration=1.25`
(H.264, CRF 18) and the short WAV (AAC). Use the same file as separate audio and
video assets; the visual clip covers 1.25 seconds at layer zero and unit opacity.
For full-length workloads, set output duration to 1,800 seconds and rate to 2 fps,
remove `trim_end`, and use either the long FLAC with a static background or
`testsrc2=size=64x64:rate=2:duration=1800` muxed with the long M4A using AAC copy.
The full visual clip covers the entire 1,800 seconds. Background is `#102030`.

## Preparation and validation

Medians in milliseconds; five measured operations per cell:

| Source | #109 preparation | Default preparation | Strict preparation |
| --- | ---: | ---: | ---: |
| Short WAV | 9.415 | 9.168 | 13.105 |
| Short FLAC | 0.863 | 0.529 | 0.878 |
| Short MP3 | 1.119 | 0.433 | 1.169 |
| Short AAC/M4A | 1.408 | 0.795 | 1.240 |
| 30-minute WAV | 109.086 | 22.166 | 114.232 |
| 30-minute FLAC | 766.552 | 0.349 | 763.152 |
| 30-minute MP3 | 1,042.075 | 0.449 | 978.576 |
| 30-minute AAC/M4A | 743.903 | 7.581 | 724.087 |
| Eight distinct 30-minute FLACs | 6,134.247 | 1.790 | 6,340.338 |
| Short video with audio | 3.901 | 2.754 | 5.204 |

Eight-source validation falls from 6,286.30 to 22.986 ms. Its remaining tool and
backend checks matter once audio inspection is cheap. Strict preparation retains
the full-decode cost. Short-file differences are small and noisy; the long-source
scaling is the useful result.

## Probe timing

Both methods run inside the same candidate binary; five measured calls follow
each method's warmup. These are timing-only microbenchmarks, excluding renderer
setup and subprocess startup. Per-method process RSS is not isolated here.

| Source | Production inspection (ms) | Strict full decode (ms) |
| --- | ---: | ---: |
| Short WAV | 9.287 | 9.489 |
| Short FLAC | 0.361 | 1.398 |
| Short MP3 | 0.540 | 1.909 |
| Short AAC/M4A | 0.757 | 1.605 |
| 30-minute WAV | 18.977 | 97.462 |
| 30-minute FLAC | 0.365 | 751.367 |
| 30-minute MP3 | 0.482 | 979.525 |
| 30-minute AAC/M4A | 6.423 | 722.344 |

The short AAC reports 1.25 seconds but decodes to 1.258667 seconds. Metadata is
therefore deliberately labeled reported, even when it is fast and positive.

## Encoding cost and memory

The eight-source excerpt demonstrates where the verification cost moves:

| Prepared worker metric | #109 | Candidate |
| --- | ---: | ---: |
| First encode, after preparation | 76.34 ms | 6,552.73 ms |
| Later encode median, five calls | 101.41 ms | 101.32 ms |
| Peak process-tree RSS | 69.39 MiB | 70.12 MiB |

Baseline preparation already decoded the sources. Default preparation defers
that work until the first audio encode; later outputs reuse verification under
the unchanged-source rule. There is no claim of an end-to-end encoding speedup
or reduced memory consumption.

Full 30-minute encoding times in seconds, selecting one measured call after
warmup on each side:

| Workflow | #109 | Candidate |
| --- | ---: | ---: |
| FLAC, one-shot | 34.339 | 33.903 |
| FLAC, prepared | 33.967 | 33.199 |
| Moving video with audio, one-shot | 141.103 | 141.053 |

The moving-video workers used 186.56 → 189.30 MiB peak process-tree RSS.
Candidate full FLAC workers peaked around 70 MiB. These small full-length sample
counts are correctness/coarse-cost evidence.

Memory uses the existing `scripts.benchmark_process.measure_process` helper,
sampling simultaneous worker-plus-descendant RSS, including FFmpeg, at 5 ms.
Shared pages may be counted twice. Memory-only repeats at 1 ms still missed four
sub-millisecond candidate preparation workers; those measurements remain
unavailable, not zero. Memory-repeat timings are discarded.

## Published-output and accuracy checks

Independent FFmpeg decoding verified all 49 retained outputs, including seven
complete 1,800-second outputs. Every full-length file has 86,400,000 decoded
samples per channel and 3,600 video frames; decoded video PTS increase strictly
from 0 to 1799.5 seconds. Matching baseline/candidate workflows have identical
full PCM and RGBA hashes. Beginning/middle/end audio windows contain the expected
signal; excerpt tails contain authored silence. The retained extra baseline
prepared-video warmup is verified output, not a completed timing worker.

Regression tests additionally cover the misleading FLAC, AAC padding, bounded
excerpts, corrupt/truncated/unsupported input, timestamp origins/gaps, playback
speed, frequency markers, source edits, cancellation and failure cleanup.
Early EOF fails before output creation; strict preparation chooses decoded
duration initially, while fast preparation keeps its reported timeline.

Raw logs, sample distributions, executable/build identities, fixture hashes and
published media remain under ignored `target/benchmark-results/audio-probing/`.
The original 80-worker excerpt matrix is retained; final default/strict
preparation, representative first/repeated encoding, probes and long outputs
were refreshed after the final bounds hardening. These Linux CPU results do not
verify Windows/macOS, dedicated GPU adapters, multi-hour output or published
files above 4 GiB.
