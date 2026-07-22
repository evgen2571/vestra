# CPU renderer baseline

This is a machine-specific release-mode baseline, not a performance guarantee.

## Workload

`cargo bench --bench animation_effects -- --nocapture` renders the canonical
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
| Total / wall time | 3,737 ms |
| Frame-render time | 3,316 ms |
| Track evaluation | 0 ms at millisecond reporting precision |
| Encoder write | 151 ms |
| Peak decoded bytes | 137,600 bytes |
| Cache peak bytes / entries | 0 bytes / 0 entries |

No comparable measurement from a prior revision is retained, so this document
establishes the baseline rather than claiming an improvement.
