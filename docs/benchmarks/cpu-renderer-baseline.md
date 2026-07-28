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
