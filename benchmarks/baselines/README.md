# CPU baseline v1

Captured on 2026-09-13 using the canonical CPU suite: 1280×720, one warmup and five measured samples per workload. Raw per-sample timings, resource counters, project/asset identities, and environment metadata are in [cpu-v1.json](cpu-v1.json).

CPU: Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz. Logical threads: 12. Release build, CPU-only features. These measurements apply to this host and environment; they are not portable performance targets.

| Workload | Frames | Median wall ms | Range ms |
| --- | ---: | ---: | ---: |
| single_video | 90 | 1664 | 1624–1671 |
| mixed_dynamic | 90 | 2586 | 2565–2912 |
| video_heavy | 90 | 8273 | 8234–8316 |
| combined | 180 | 10454 | 10402–10528 |
| masks | 30 | 448 | 443–454 |
| mattes | 30 | 312 | 303–323 |
| nested_groups | 90 | 4616 | 4584–4657 |
| particles | 30 | 862 | 854–872 |
| blend_modes | 150 | 736 | 736–741 |
| production_edit | 360 | 14535 | 14502–14679 |

Base revision: `953d10e8fd86be04ef0495003626ec1f75119081`. The capture includes uncommitted benchmark changes, recorded as dirty. Its source fingerprint is `d1290f3d732188c0991d5707df6da59a0f5c14fc3830919e3a88d0f73a68dbb0` and executable SHA-256 is `69b4da4f24d7e55b14bfc6de63f20191b66478e65b84d931e0e9674946d7455d`. Subsequent Python changes tighten suite validation; they do not change the measured Rust executable.

Reproduce with:

```bash
python scripts/benchmark.py run --suite canonical --backend cpu --output target/benchmark-results/new-baseline
python scripts/benchmark.py compare benchmarks/baselines/cpu-v1.json target/benchmark-results/new-baseline/suite.json
```

The comparison deliberately rejects different environments. On another host, capture a new local baseline before optimizing. Keep this record immutable; add a new named baseline when the workload definition changes. All ten workloads completed, and all 1,020 available timing/resource metrics passed self-comparison. The unused-asset warnings in the two original video scenarios are intentional existing coverage.

The production workload uses moving synthetic footage and an audio tone, with realistic edit operations. The baseline includes asset preparation and encoding, but excludes fixture generation. OS cache state is uncontrolled. Hardware WGPU has not been measured on this host; this file contains only CPU results.
