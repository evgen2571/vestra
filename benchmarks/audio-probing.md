# Bounded audio execution results

These measurements support [#110](https://github.com/evgen2571/vestra/issues/110)
and [#111](https://github.com/evgen2571/vestra/pull/111). For a one-second excerpt
from eight 30-minute FLACs, preparation plus the first prepared encode falls from
6.13 seconds in #109 and 6.16 seconds in the original #111 to 79 milliseconds in
the measured revision. Its one-shot median falls from 6.18/6.13 seconds to
127 milliseconds. Explicit strict preparation retains the full-source audit cost.

**Revision status:** the revised measurements below are for `90644a8`. A macOS
AAC decoder mismatch is under investigation; final-candidate refresh is pending.
These numbers and output checks describe the recorded Linux revision only.

## Environment and protocol

| Version | Production revision | Policy |
| --- | --- | --- |
| #109 | `9eb6801599bbb6b89c566866bf2b4e8060557035` | Full decoding during preparation |
| Original #111 | `4f7ff02db7c3aede0e6c27b82d7e05955a7e394a` | Metadata preparation; separate full verification at first prepared encode |
| Revised | `90644a8eb93df71a34001eb0c5a44bbb6ea993dd` | Count consumed source samples during execution; resolve omitted ends exactly |

- Linux x86_64, AMD Ryzen 5 3600 (6 cores / 12 threads), Rust 1.95.0,
  release builds, selected CPU renderer, preview quality, warm OS cache.
- All three newly built executables link the same native FFmpeg 8.1.3 release
  libraries, source `a35157abb96167fd7308edc92b55408d5bb084b0`, with the same
  configuration and actual static-library directory. External fixture generation,
  encoding and independent decoding use FFmpeg 8.1.2.
- Run workers serially without concurrent compilation or benchmarks. Loading JSON
  and fixture generation are outside operation intervals. Source media are
  unchanged from the previous capture; new projects reference those files.
- Eight default workloads × three operations × three versions, plus three strict
  prepared workloads × three versions: 81 workers. Each worker records its first
  operation and five later operations. Standalone preparation starts afresh each
  time; one-shot rendering includes preparation every time.
- Prepared workers prepare once, then encode six times. The harness records
  `preparation_wall_ms` separately. Preparation, first encode and their sum are
  single observations, not medians. Later encode and one-shot figures are medians
  and minimum–maximum ranges of five samples. The first encode is also the harness
  warmup and is deliberately retained to expose deferred work.

The benchmark-only harness is copied into each baseline, removing only
`.with_strict_audio(strict_audio)` calls for #109, where that API did not exist.
Before each build, clean release artifacts for `vestra-core`, `vestra-media` and
`vestra` to avoid shared-target reuse. Preserved executable SHA-256 values:

| Version | SHA-256 |
| --- | --- |
| #109 | `7a79ccc4b1d5313503415f1d8714d301baf33d35dfb7149f000729ec1d2bb8ad` |
| Original #111 | `a9f434df4a21cea1ec13af525e09bba2a597d6d0801580b8bd4ac5a7be5b3527` |
| Revised | `b262a63b2688a29ea83ab85f9f58b12590cd0655d9d9bf46e8f02d19d5985b0b` |

## Workloads and reproduction

Use the [benchmark commands](README.md) and ignored
`audio_workflow_benchmark` test. Audio fixtures are stereo 48 kHz: short sources
are 1.25 seconds and long sources 1,800 seconds. Generate formats from the same
440 Hz tone, selecting `pcm_s16le`/WAV, `flac`/FLAC, `libmp3lame`/MP3 or `aac`/M4A:

```bash
ffmpeg -nostdin -v error -f lavfi \
  -i 'sine=frequency=440:sample_rate=48000:duration=1800' \
  -ac 2 -c:a flac long.flac
```

The eight-source workload uses distinct files at `440 + 37 * index` Hz, indices
0–7, each with gain 1/8. Normal projects have a 64×64, 24 fps output, explicit
1.25-second duration and background `#102030`. Audio starts at timeline/source
zero, with `trim_end: 1`; the last 0.25 seconds are authored silence. The
misleading FLAC excerpt uses `trim_end: 0.25`. These are bounded excerpts, not
full-length encoding throughput measurements.

Short AV uses `testsrc2=size=64x64:rate=24:duration=1.25` encoded with H.264 CRF 18
and AAC. Long AV uses `testsrc2=size=64x64:rate=2:duration=1800` with the long M4A
copied into the MP4. Each AV project references the same file as separate video
and audio assets; the visual selection covers the 1.25-second output.

The AAC automatic-duration case uses the short M4A, `trim_start: 0.25`, clip
start 0.125, omitted `trim_end`, automatic output duration and 8 fps. This case
must reach source EOF to retain exact planning. The misleading FLAC control has
10 seconds in metadata but only 0.375 seconds of decoded audio; its automatic
project has no trim bounds. Keep both controls when checking executable identity.

## Preparation and first prepared output

Milliseconds. `Prepare + first = total` includes the once-only preparation
outside encode timing. All rows use default preparation; #109 has no strict flag.

| Workload | Version | Prepare | First encode | Total first output | Later encode median (range) |
| --- | --- | ---: | ---: | ---: | ---: |
| Short FLAC | #109 | 2.13 | 76.01 | 78.13 | 76.22 (76.09–101.15) |
| Short FLAC | Original #111 | 1.40 | 77.27 | 78.68 | 76.14 (75.94–101.11) |
| Short FLAC | Revised | 1.07 | 76.15 | 77.22 | 101.29 (76.07–101.48) |
| Short MP3 | #109 | 4.00 | 76.12 | 80.12 | 76.14 (75.95–76.23) |
| Short MP3 | Original #111 | 2.93 | 77.16 | 80.09 | 76.40 (76.23–101.33) |
| Short MP3 | Revised | 2.25 | 76.16 | 78.41 | 76.36 (76.19–76.47) |
| 30-minute FLAC | #109 | 746.11 | 101.26 | 847.37 | 76.23 (76.01–76.39) |
| 30-minute FLAC | Original #111 | 1.11 | 833.78 | 834.89 | 76.40 (76.28–101.40) |
| 30-minute FLAC | Revised | 1.66 | 76.44 | 78.10 | 76.48 (76.16–101.79) |
| Eight 30-minute FLACs | #109 | 6057.07 | 76.15 | 6133.22 | 101.15 (76.02–101.29) |
| Eight 30-minute FLACs | Original #111 | 5.24 | 6155.42 | 6160.66 | 76.25 (76.08–101.66) |
| Eight 30-minute FLACs | Revised | 3.07 | 76.17 | 79.24 | 101.23 (101.20–101.31) |
| Short AV | #109 | 5.74 | 59.21 | 64.95 | 59.03 (58.12–69.06) |
| Short AV | Original #111 | 5.32 | 60.40 | 65.72 | 57.30 (54.45–70.78) |
| Short AV | Revised | 4.53 | 59.04 | 63.57 | 58.14 (57.11–59.02) |
| 30-minute AV excerpt | #109 | 732.78 | 78.73 | 811.51 | 72.34 (68.51–74.93) |
| 30-minute AV excerpt | Original #111 | 19.48 | 831.70 | 851.18 | 68.03 (67.27–70.13) |
| 30-minute AV excerpt | Revised | 22.99 | 78.19 | 101.18 | 71.44 (70.21–74.88) |
| AAC automatic duration | #109 | 3.25 | 75.98 | 79.23 | 76.13 (76.03–76.23) |
| AAC automatic duration | Original #111 | 2.61 | 77.48 | 80.08 | 76.01 (75.96–76.18) |
| AAC automatic duration | Revised | 5.35 | 76.40 | 81.75 | 76.17 (76.04–76.21) |
| Misleading FLAC excerpt | #109 | 1.19 | 50.87 | 52.07 | 51.01 (50.89–75.95) |
| Misleading FLAC excerpt | Original #111 | 1.02 | 51.15 | 52.17 | 76.06 (51.02–76.40) |
| Misleading FLAC excerpt | Revised | 0.95 | 50.92 | 51.87 | 51.34 (51.09–76.20) |

The original PR moves long-source decoding from preparation to the first
prepared encode. The revision avoids that separate scan for these bounded
selections. It does not claim whole-source integrity after a prefix read.
Repeated prepared calls still perform the necessary audio encode; their small
differences are noisy. Static-output encoder completion is polled at roughly
25 ms intervals, visible in the short-call ranges. Five samples from one run do
not establish a statistically significant short-file speed change.

Fresh-preparation medians for one/eight long FLACs are 750.95/6,062.48 ms in #109,
0.35/1.71 ms in the original PR and 0.47/1.71 ms in the revision. These differ
from the single first-preparation observations above because each worker starts
with fresh library/runtime state. Exact automatic AAC preparation has a revised
median of 2.36 ms versus 0.67 ms in the original PR: restoring the exact omitted
source end intentionally costs a decode.

## One-shot output

Milliseconds, median (minimum–maximum), five measured calls after warmup.

| Workload | #109 | Original #111 | Revised |
| --- | ---: | ---: | ---: |
| Short FLAC | 102.58 (97.53–124.46) | 103.13 (97.16–130.36) | 121.72 (96.67–174.16) |
| Short MP3 | 97.75 (96.71–98.97) | 97.59 (96.19–122.65) | 96.09 (95.58–104.16) |
| 30-minute FLAC | 865.83 (841.61–907.27) | 846.07 (840.79–865.16) | 97.78 (95.55–122.61) |
| Eight 30-minute FLACs | 6181.58 (6089.10–6194.08) | 6130.88 (6054.92–6230.56) | 127.14 (122.95–136.17) |
| Short AV | 84.11 (82.63–86.78) | 85.15 (79.73–88.64) | 82.64 (80.50–92.05) |
| 30-minute AV excerpt | 843.44 (827.60–874.10) | 853.30 (833.90–865.13) | 123.74 (114.31–139.02) |
| AAC automatic duration | 97.92 (96.92–101.20) | 99.29 (73.01–105.73) | 98.39 (96.69–101.73) |
| Misleading FLAC excerpt | 70.43 (70.32–70.66) | 97.36 (73.15–103.54) | 70.74 (70.65–101.40) |

The default-prepared AAC row in the previous table produces a different
nine-frame result in the original PR, so it is diagnostic rather than an
equivalent-output speed comparison. The one-shot row here compares matching
ten-frame outputs, as do strict preparation and all revised modes.

## Strict preparation

Milliseconds. #109's ordinary preparation already fully decodes, so its rows
are full-audit references despite the unavailable strict API. Prepared encode
ranges contain five later calls; preparation/first/total remain single samples.

| Workload | Version | Prepare | First encode | Total first output | Later encode median (range) | Peak RSS (MiB) |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 30-minute FLAC | #109 | 755.74 | 76.29 | 832.02 | 76.13 (76.09–76.20) | 57.63 |
| 30-minute FLAC | Original #111 | 766.59 | 101.43 | 868.02 | 76.11 (76.02–103.98) | 58.11 |
| 30-minute FLAC | Revised | 752.94 | 76.34 | 829.28 | 76.34 (76.16–101.31) | 58.58 |
| Eight 30-minute FLACs | #109 | 6068.54 | 76.28 | 6144.82 | 76.19 (76.08–76.26) | 69.50 |
| Eight 30-minute FLACs | Original #111 | 6023.97 | 127.16 | 6151.14 | 101.16 (75.99–101.32) | 70.24 |
| Eight 30-minute FLACs | Revised | 6054.99 | 101.41 | 6156.41 | 101.29 (101.16–101.46) | 72.64 |
| AAC automatic duration | #109 | 2.94 | 51.03 | 53.97 | 76.10 (76.00–76.19) | 59.64 |
| AAC automatic duration | Original #111 | 2.85 | 75.94 | 78.79 | 76.11 (76.06–76.16) | 59.46 |
| AAC automatic duration | Revised | 3.22 | 76.29 | 79.51 | 76.11 (76.06–76.40) | 59.92 |

The eight-source strict total stays near 6.15 seconds in all versions. Strict
full-source audit and exact EOF-dependent planning remain distinct from bounded
execution; the [audio contract](../docs/reference/audio.md) defines their guarantees.

## Process memory

Peak simultaneous worker-plus-descendant RSS, MiB. Each cell is
`prepared worker / one-shot worker`; the prepared peak covers preparation plus
all six encodes, not just one call.

| Workload | #109 | Original #111 | Revised |
| --- | ---: | ---: | ---: |
| Short FLAC | 57.48 / 57.66 | 58.05 / 58.09 | 58.39 / 58.43 |
| Short MP3 | 57.66 / 57.93 | 58.30 / 58.40 | 58.59 / 58.74 |
| 30-minute FLAC | 58.36 / 58.27 | 58.48 / 58.82 | 58.79 / 58.59 |
| Eight 30-minute FLACs | 69.62 / 69.26 | 70.10 / 70.04 | 71.98 / 71.90 |
| Short AV | 74.25 / 76.16 | 75.86 / 76.54 | 74.58 / 77.21 |
| 30-minute AV excerpt | 126.87 / 127.66 | 127.45 / 131.53 | 127.22 / 130.77 |
| AAC automatic duration | 58.99 / 59.21 | 60.27 / 60.05 | 59.89 / 60.15 |
| Misleading FLAC excerpt | 57.03 / 57.24 | 57.85 / 57.62 | 57.88 / 57.96 |

Use the existing `scripts.benchmark_process.measure_process` helper with psutil
7.2.2 and 5 ms sampling. FFmpeg descendants are included; shared pages may be
counted twice. Some sub-millisecond standalone preparation workers escape
sampling, so those raw RSS values are unavailable, never zero. Encoded/prepared
workers have observed peaks. Eight-source prepared RSS rises by 2.36 MiB versus
#109 and 1.88 MiB versus the original PR. There is no memory-saving claim, and
these short workers do not prove sustained memory stability.

## Published output and duration controls

Independent FFmpeg decoding checked all 62 new published outputs, streaming each
complete stereo float PCM and RGBA payload into hashes. All equivalent outputs
match across versions, one-shot/default-prepared/strict-prepared modes; the
original default-prepared AAC difference is explicitly excluded from equivalence.
Checks also cover sample counts, tone/silence windows, container duration on the
video frame grid, decoded video frame counts and finite, strictly increasing PTS
from zero to `(frame_count - 1) / fps`.

- Normal excerpts publish 30 frames over 1.25 seconds and decode to 60,416 audio
  samples per channel, including AAC output padding. Signal appears in beginning
  and middle windows; the 1.1–1.2 second window is silent.
- Automatic AAC has exact project duration 1.133666667 seconds, ten frames at
  8 fps, last video PTS 1.125 seconds and 55,296 decoded audio samples per channel
  in #109, original one-shot/strict and every revised mode. Its MP4 duration is
  1.25 seconds after frame-grid rounding. Original default-prepared instead has
  project/container duration 1.125 seconds, nine frames, last PTS 1.0 second and
  54,272 decoded audio samples per channel. Initial silence and subsequent tone
  placement are verified in both results.
- The misleading automatic FLAC prepares/renders at 0.375 seconds in #109 and
  the revision. Original default preparation reports 10 seconds and emits
  `VESTRA-AUDIO-DURATION-REPORTED`; its prepared encode correctly fails with
  `VESTRA-AUDIO-CLIP-SOURCE` and publishes nothing, while original one-shot renders
  0.375 seconds. Successful controls publish nine frames, 18,432 decoded audio
  samples per channel and matching complete PCM/RGBA hashes. Revised inspection
  retains provenance without the normal-metadata warning.

The 62 files contain 3,488,768 decoded samples per channel and 1,574 video frames
in total. This validates bounded outputs and exact-duration controls, not
unread source tails. Truncated/corrupt sources, timestamp gaps, codec padding,
speed, cancellation, sample-counter failures and publication cleanup are covered
by the repository's focused regressions, separately from these timing workers.

## Retained evidence and limits

New raw records, per-worker logs, fixture hashes, actual FFmpeg build-script
records, source/executable identities and output-verification details remain under
ignored `target/benchmark-results/audio-bounded-execution/`. All 18 source-file
hashes were recorded; the earlier `audio-probing/` evidence remains untouched.

The earlier capture also verified 49 outputs including seven complete
1,800-second files, with 86,400,000 decoded samples per channel and 3,600 video
frames for each full output. Those are historical #109/original-PR results,
not refreshed production-revision measurements. They establish no full-length
speedup for this revision. The follow-up deliberately avoids repeating expensive
30-minute encodes when short matched outputs isolate the changed verification
cost.

These results are Linux CPU evidence. They do not verify Windows/macOS, dedicated
GPU adapters, multi-hour output or files larger than 4 GiB. Final candidate and
cross-platform status must be refreshed after the pending decoder fix.
