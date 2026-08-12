# CPU renderer baseline

This is a machine-specific release-mode baseline, not a performance guarantee.

## Workload

`cargo bench -p video-editor --bench animation_effects -- --nocapture` renders the canonical
animation/effects example at 720×1280, 24 FPS, for 2.5 seconds (60 frames).
It includes animated position, scale, rotation, a crossfade, a flash overlay,
and brightness, contrast, saturation, and tint effects. The output uses the
balanced H.264 settings (CRF 23) and a 256 MiB crop-cache budget.

## Recorded baseline

Recorded on 2026-07-22:

| Field | Value |
| --- | --- |
| OS | Debian GNU/Linux 13, kernel 7.0.0-27-generic |
| CPU | 12 logical CPUs (model not reported by this environment) |
| RAM | 14 GiB |
| Rust | rustc 1.96.1 |
| FFmpeg | 7.1.5 |
| Build profile | Cargo `bench` (optimized release profile) |
| Total / wall time | 3,945 ms |
| Frame-render time | 3,498 ms |
| Track evaluation | 0 ms at millisecond reporting precision |
| Encoder write | 159 ms |
| Peak decoded bytes | 137,600 bytes |
| Cache peak bytes / entries | 0 bytes / 0 entries |

No comparable measurement from a prior revision is retained, so this document
establishes the baseline rather than claiming an improvement.

## Refactor verification

On 2026-07-25, after the architecture refactor, the reduced development command
below ran on the same 720×1280 basic-colour workload:

```bash
VIDEO_EDITOR_BENCH_BACKEND=cpu VIDEO_EDITOR_BENCH_WARMUPS=0 \
VIDEO_EDITOR_BENCH_SAMPLES=1 cargo bench -p video-editor --bench animation_effects -- --nocapture
```

| Field | 2026-07-23 baseline | Refactor check | Difference |
| --- | ---: | ---: | ---: |
| Wall time | 11,398 ms | 11,505 ms | +0.9% |
| Frame-render time | 10,882 ms | 11,015 ms | +1.2% |
| Effective FPS | 12.63 | 12.52 | -0.9% |
| Encoder write | not recorded | 288 ms | n/a |
| Rendered frames | 60 | 60 | unchanged |
| Cache peak bytes / entries | 0 / 0 | 0 / 0 | unchanged |

This is a one-sample development comparison, so it catches obvious extra work
without claiming a release-performance result. The refactor remains well below
the 10% regression threshold.

## Multi-core CPU phase measurement

On 2026-08-12, the ignored release-mode direct backend benchmark used the same
1920×1080, 60-frame evaluated animation/effects workload for 1, 2, 4, and auto
(8) workers:

| Workers | Wall ms | Effective FPS | Aggregate frame-render ms | Peak in-flight | Scratch retained bytes |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 12,372.512 | 4.85 | 12,368.447 | 1 | 24,883,200 |
| 2 | 6,215.874 | 9.65 | 12,405.502 | 2 | 49,766,400 |
| 4 | 3,181.688 | 18.86 | 12,671.340 | 4 | 99,532,800 |
| auto (8) | 2,077.725 | 28.88 | 15,989.852 | 8 | 199,065,600 |

The canonical end-to-end CPU benchmark at 1920×1080 (one measured sample,
zero warmups) rendered 144 frames in 6,938 ms at 20.76 effective FPS, with
actual backend pipeline depth 8. Optional CPU-utilization and RSS measurements
were not collected.
