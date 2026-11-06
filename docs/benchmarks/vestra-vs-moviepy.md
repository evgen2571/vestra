# Vestra vs MoviePy v2 portable benchmark investigation

Run date: 2026-10-31 (UTC). Baseline: `develop` at `34dec665090aa5706a83eb8302d38f5d1801dbf5` (clean).

These are historical measurements of that revision and environment, not current
Vestra performance claims. No production or benchmark source was changed during
the investigation. The raw collection and suite JSON/logs were retained locally
under `target/benchmark-results/`; they are not included in a clean checkout.
Ratios below are wall-time ratios (candidate / MoviePy CPU for matched workloads;
hardware WGPU / CPU for Vestra-only pairs), so values below 1.00x are faster.

## Environment and fairness

- Host: AMD Ryzen 5 3600 (12 logical CPUs), 16 GiB RAM, Linux 6.18.59 x86_64, all CPUs 0–11 available.
- Toolchain: Rust 1.95.0, Cargo 1.95.0, Python 3.13.17, uv 0.11.21, FFmpeg/ffprobe 8.1.2 (libx264), release profile.
- Python packages: Vestra 0.1.3, MoviePy 2.2.1, NumPy 2.5.4, Pillow 11.3.0, psutil 7.2.2, imageio 2.38.1, imageio-ffmpeg 0.6.0.
- WGPU: Vulkan NVIDIA GeForce GTX 1650 SUPER, discrete GPU, driver 595.71.05; llvmpipe was discovered but rejected as software.
- CPU and WGPU runs used identical generated fixtures, suite definitions, 30 fps settings, FFmpeg/libx264 output validation, and serial execution. The MoviePy comparison used the same prepared forest/valley/tear inputs and 640x360 matched timelines.
- Process resources are simultaneous process-tree RSS sampled by psutil every 20 ms for short comparisons and every 5 s for sustained runs. PSS was unavailable in the harness, so RSS is reported and is a lower-bound/shared-page-sensitive metric; GPU VRAM is not measured. CPU utilization is observed process-tree CPU time, with one core = 100%.

## Reproduction commands

From an existing clone, fetch the history and create a separate checkout of the
recorded revision. Choose a new worktree directory; these commands preserve
your current branch and working files:

```bash
git fetch origin develop
git worktree add --detach ../vestra-benchmark-34dec665 34dec665090aa5706a83eb8302d38f5d1801dbf5
cd ../vestra-benchmark-34dec665
nix develop .#comparison
# Keep Cargo artifacts separate from any other checkout.
export CARGO_TARGET_DIR="$PWD/target"
uv run --no-project --python "$(command -v python3)" scripts/collect-benchmarks.py --profile all --hours 1 --suite canonical --backend auto --graphics-api vulkan --timeout 900 --sustained-timeout 14400 --output target/benchmark-results/full-collection
```

Use the Nix shell's Python so the interpreter and native extension share
runtime libraries. The collector builds its own executables and creates a
uniquely named result directory. To run an additional suite from the tables below, use the runner;
this example selects `stylized-effects-1080p` and builds the matching executable:

```bash
VESTRA_BENCH_VALIDATE=1 uv run --no-project --python "$(command -v python3)" python scripts/benchmark.py run --suite stylized-effects-1080p --backend cpu --output target/benchmark-results/complete-suites/cpu/stylized-effects-1080p
VESTRA_BENCH_VALIDATE=1 VESTRA_WGPU_BACKEND=vulkan VESTRA_REQUIRE_HARDWARE_WGPU=1 uv run --no-project --python "$(command -v python3)" python scripts/benchmark.py run --suite stylized-effects-1080p --backend hardware-wgpu --output target/benchmark-results/complete-suites/hardware-wgpu/stylized-effects-1080p
for benchmark in preparation_matrix effect_scaling_matrix random_access_matrix; do
  VESTRA_RENDER_BENCH=1 cargo test --release --locked -p vestra --no-default-features --features cpu "$benchmark" -- --nocapture --test-threads=1
done
```

Use a fresh output directory when repeating a direct suite. Hardware commands
require a verified eligible adapter; CPU runs can be collected separately.
Toolchain, driver and host differences can change the results even at the same
revision. For current-checkout measurements and native setup, follow
[benchmark collection](https://github.com/evgen2571/vestra/blob/develop/benchmarks/COLLECTING.md); keep new results separate
from these historical tables.

The historical collector archive was `target/benchmark-results/full-collection/vestra-bench-linux-20261031T130534Z-180e48c2.zip`. The direct suite executable hashes were recorded in each `suite.json` provenance and the collector diagnostics.

## Matched MoviePy v2 workloads

The three matched scenarios are 640x360, 30 fps, 10 s (300 frames), silent H.264/yuv420p, CRF 23/veryfast. Inputs and output probes were identical. The verifier passed 135/135 sampled frames/regions; all nine output streams passed full decode and structural checks. Resampling/rounding differences are expected and are covered by the suite tolerance.

| Scenario | MoviePy CPU wall s | Vestra CPU wall s | CPU ratio | Vestra WGPU wall s | WGPU ratio | MoviePy FPS | Vestra CPU FPS | Vestra WGPU FPS | MoviePy RSS MiB | Vestra CPU RSS MiB | WGPU RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| basic | 0.997 | 0.835 | 0.84x | 1.064 | 1.07x | 300.84 | 359.25 | 281.82 | 213.2 | 445.9 | 567.9 |
| editing | 8.652 | 1.759 | 0.20x | 1.239 | 0.14x | 41.61 | 204.67 | 290.62 | 379.6 | 567.1 | 636.7 |
| layered | 4.227 | 1.789 | 0.42x | 1.305 | 0.31x | 70.97 | 167.71 | 229.85 | 365.3 | 599.5 | 668.5 |

Variability is retained in the raw JSON. In the matched table, MoviePy CPU wall-time ranges were 1.00–1.11 s (basic), 8.35–8.85 s (editing), and 4.17–4.23 s (layered); Vestra CPU ranges were 0.83–0.89 s, 1.73–1.86 s, and 1.75–1.86 s. WGPU ranges are similarly recorded in `comparison.json`.

## Complete Vestra suite results

Each row is the median of the versioned suite samples (canonical and collector community/canonical runs: 1 warmup + 5 or 3 samples as recorded; direct 1080p/community runs: 1 warmup + 3 samples). The smoke and subset suites in `suites.json` are aliases/control reductions of these unique workloads and were not rerun as duplicate performance captures. `wall` includes project preparation, decoding, frame work, encoding and publication. `frame_sum` is Vestra’s summed frame-work counter and can exceed wall time because worker work overlaps.

| Suite | Scenario | Resolution | CPU wall s | CPU FPS | WGPU wall s | WGPU FPS | WGPU/CPU |
|---|---|---:|---:|---:|---:|---:|---:|
| canonical | single_video | 1280x720 | 0.770 | 116.88 | 0.762 | 118.11 | 0.99x |
| canonical | mixed_dynamic | 1280x720 | 1.153 | 78.06 | 0.905 | 99.45 | 0.78x |
| canonical | video_heavy | 1280x720 | 3.256 | 27.64 | 8.985 | 10.02 | 2.76x |
| canonical | combined | 1280x720 | 5.400 | 33.33 | 0.910 | 197.80 | 0.17x |
| canonical | masks | 1280x720 | 0.289 | 103.81 | 0.449 | 66.82 | 1.55x |
| canonical | mattes | 1280x720 | 0.249 | 120.48 | 0.491 | 61.10 | 1.97x |
| canonical | nested_groups | 1280x720 | 2.070 | 43.48 | 0.668 | 134.73 | 0.32x |
| canonical | particles | 1280x720 | 0.251 | 119.52 | 0.405 | 74.07 | 1.61x |
| canonical | blend_modes | 1280x720 | 0.619 | 242.33 | 0.683 | 219.62 | 1.10x |
| canonical | production_edit | 1280x720 | 6.543 | 55.02 | 3.099 | 116.17 | 0.47x |
| community-v1 | single_video | 640x360 | 0.235 | 382.98 | 0.354 | 254.24 | 1.51x |
| community-v1 | mixed_dynamic | 640x360 | 0.316 | 284.81 | 0.384 | 234.38 | 1.22x |
| community-v1 | video_heavy | 640x360 | 0.901 | 99.89 | 1.287 | 69.93 | 1.43x |
| community-v1 | masks | 640x360 | 0.140 | 214.29 | 0.316 | 94.94 | 2.26x |
| community-v1 | mattes | 640x360 | 0.129 | 232.56 | 0.390 | 76.92 | 3.02x |
| community-v1 | production_edit | 640x360 | 2.124 | 169.49 | 1.605 | 224.30 | 0.76x |
| row-copy-1080p | opaque_video | 1920x1080 | 4.629 | 64.81 | 4.876 | 61.53 | 1.05x |
| row-copy-1080p | ineligible_video | 1920x1080 | 10.644 | 28.18 | 4.644 | 64.60 | 0.44x |
| stylized-effects-1080p | stylization_baseline | 1920x1080 | 2.086 | 43.14 | 1.840 | 48.91 | 0.88x |
| stylized-effects-1080p | palette_video | 1920x1080 | 2.863 | 31.44 | 1.780 | 50.56 | 0.62x |
| stylized-effects-1080p | dither_video | 1920x1080 | 2.991 | 30.09 | 1.836 | 49.02 | 0.61x |
| stylized-effects-1080p | ascii_video | 1920x1080 | 6.345 | 14.18 | 1.728 | 52.08 | 0.27x |
| stylized-effects-1080p | halftone_video | 1920x1080 | 6.007 | 14.98 | 1.707 | 52.72 | 0.28x |
| stylized-effects-1080p | sort_horizontal_video | 1920x1080 | 2.802 | 32.12 | 1.748 | 51.49 | 0.62x |
| stylized-effects-1080p | sort_vertical_video | 1920x1080 | 2.823 | 31.88 | 1.756 | 51.25 | 0.62x |
| stylized-effects-1080p | crt_video | 1920x1080 | 5.745 | 15.67 | 1.798 | 50.06 | 0.31x |
| dither-patterns-1080p | stylization_baseline | 1920x1080 | 2.063 | 43.63 | 1.772 | 50.79 | 0.86x |
| dither-patterns-1080p | dither_video | 1920x1080 | 3.015 | 29.85 | 1.781 | 50.53 | 0.59x |
| dither-patterns-1080p | blue_noise_video | 1920x1080 | 2.915 | 30.87 | 1.806 | 49.83 | 0.62x |
| chromatic-dither-1080p | stylization_baseline | 1920x1080 | 2.123 | 42.39 | 1.717 | 52.42 | 0.81x |
| chromatic-dither-1080p | blue_noise_video | 1920x1080 | 3.013 | 29.87 | 1.775 | 50.70 | 0.59x |
| chromatic-dither-1080p | rgb_dither_video | 1920x1080 | 3.602 | 24.99 | 1.823 | 49.37 | 0.51x |
| chromatic-dither-1080p | hue_dither_video | 1920x1080 | 3.782 | 23.80 | 1.773 | 50.76 | 0.47x |
| channel-dither-1080p | blue_noise_video | 1920x1080 | 3.066 | 29.35 | 1.814 | 49.61 | 0.59x |
| channel-dither-1080p | channel_dither_video | 1920x1080 | 3.373 | 26.68 | 1.801 | 49.97 | 0.53x |
| perceptual-dither-1080p | blue_noise_video | 1920x1080 | 3.016 | 29.84 | 1.802 | 49.94 | 0.60x |
| perceptual-dither-1080p | rgb_dither_video | 1920x1080 | 3.525 | 25.53 | 1.791 | 50.25 | 0.51x |
| perceptual-dither-1080p | oklab_dither_video | 1920x1080 | 4.860 | 18.52 | 1.845 | 48.78 | 0.38x |
| tonal-stops-1080p | blue_noise_video | 1920x1080 | 3.077 | 29.25 | 1.791 | 50.25 | 0.58x |
| tonal-stops-1080p | nonuniform_dither_video | 1920x1080 | 3.186 | 28.25 | 1.811 | 49.70 | 0.57x |
| input-detail-1080p | tone_dither_video | 1920x1080 | 3.315 | 27.15 | 1.740 | 51.72 | 0.52x |
| input-detail-1080p | detail_dither_video | 1920x1080 | 4.913 | 18.32 | 1.820 | 49.45 | 0.37x |
| input-detail-1080p | contrast_dither_video | 1920x1080 | 7.654 | 11.76 | 1.784 | 50.45 | 0.23x |
| input-tone-1080p | blue_noise_video | 1920x1080 | 2.931 | 30.71 | 1.806 | 49.83 | 0.62x |
| input-tone-1080p | tone_dither_video | 1920x1080 | 3.047 | 29.54 | 1.779 | 50.59 | 0.58x |
| palette-interpolation-1080p | palette_video | 1920x1080 | 2.797 | 32.18 | 1.766 | 50.96 | 0.63x |
| palette-interpolation-1080p | oklab_palette_video | 1920x1080 | 4.642 | 19.39 | 1.752 | 51.37 | 0.38x |

Notable effects: on CPU at 1920x1080, `ascii_video` is 6.345 s (14.18 FPS), `halftone_video` 6.007 s (14.98 FPS), `crt_video` 5.745 s (15.67 FPS), `contrast_dither_video` 7.654 s (11.76 FPS), and `oklab_palette_video` 4.642 s (19.39 FPS). On the same GPU these effects cluster at 1.71–1.85 s (roughly 49–53 FPS), so the CPU effect kernels are the dominant backend-specific loss.

Across the direct repeated-sample rows, the median min–max wall-time spread is 2.7% for both CPU and hardware WGPU. The widest observed spreads were 17.9% for the short 640x360 CPU `community-v1/single_video` control and 14.8% for hardware-WGPU 1080p `row-copy-1080p/opaque_video`; exact ranges and standard deviations remain in each `suite.json`.

### 4K resource/correctness smoke

The existing direct 4K smoke used `dither_video`, 3840x2160, 90 frames, zero warmups and one measured sample. It is a resource/correctness check, not a repeated baseline. Both outputs completed the benchmark validation path.

| Backend | Wall s | FPS | Frame sum ms | Encoder write ms | Texture uploads | Staging bytes | Row repack ms | Adapter |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| CPU | 23.583 | 3.82 | 42909 | 3083 | 0 | 0 | n/a | none |
| Hardware WGPU | 6.029 | 14.93 | 2838 | 2850 | 1,957,478,400 bytes | 398,134,272 | 857 | NVIDIA GTX 1650 SUPER / Vulkan |

The collector also passed the 128x72 canonical smoke checks; those one-sample smoke timings are execution evidence only and are not used as performance baselines.

## Stage and resource evidence

| Workload/backend | Wall s | Frame sum ms | Encoder write ms | Encoder finalize ms | Relevant counters |
|---|---:|---:|---:|---:|---|
| canonical/cpu/video_heavy | 3.256 | 24336 | 268 | 21 | cpu_full_frame_allocations=90; cpu_scratch_bytes_retained=29491200 |
| canonical/cpu/combined | 5.400 | 41456 | 629 | 15 | cpu_full_frame_allocations=180; cpu_scratch_allocations=24; cpu_scratch_bytes_retained=88473600 |
| canonical/cpu/production_edit | 6.543 | 48653 | 1322 | 62 | cpu_full_frame_allocations=360; cpu_scratch_allocations=56; cpu_scratch_bytes_retained=58982400 |
| stylized-effects-1080p/cpu/ascii_video | 6.345 | 46782 | 613 | 67 | cpu_full_frame_allocations=90; cpu_scratch_allocations=16; cpu_scratch_bytes_retained=199065600 |
| input-detail-1080p/cpu/contrast_dither_video | 7.654 | 57313 | 611 | 75 | cpu_full_frame_allocations=90; cpu_scratch_allocations=16; cpu_scratch_bytes_retained=199065600 |
| row-copy-1080p/cpu/opaque_video | 4.629 | 28378 | 3179 | 45 | cpu_opaque_copy_fast_path_hits=600; cpu_opaque_copy_fast_path_bytes=4354560000; cpu_full_frame_allocations=300; cpu_scratch_bytes_retained=66355200 |
| row-copy-1080p/cpu/ineligible_video | 10.644 | 80601 | 1886 | 63 | cpu_full_frame_allocations=300; cpu_scratch_bytes_retained=66355200 |
| stylized-effects-1080p/hardware-wgpu/ascii_video | 1.728 | 865 | 536 | 64 | none |
| stylized-effects-1080p/hardware-wgpu/crt_video | 1.798 | 755 | 543 | 60 | none |

The raw Vestra records report `asset_decode_ms=0` for these deterministic benchmark fixtures; decoder work is represented by the video metrics/counters and the separate cache/access harness. MoviePy exposes no equivalent internal decode/composite stage timings, so stage-level cross-engine attribution is not claimed.

Encoding is visible but is not isolated as a Vestra-specific loss: the same libx264/veryfast/yuv420p contract is used by both engines in the matched comparison, and Vestra’s encoder-write counters are 0.3–3.2 s in the selected CPU rows and about 0.5–0.6 s in the selected WGPU effect rows. Because frame work, encoder writes and worker queues overlap, these counters must not be added to wall time. The measured evidence points to CPU compositing/effects and GPU readback rather than an encoder regression. Decoding is likewise not a demonstrated end-to-end bottleneck here (`asset_decode_ms` is zero in the deterministic records); the cache/access results are the available decoder evidence and show pattern-dependent reuse benefits.

## Cache and sustained scenarios

| Cache request pattern | Vestra cache off ms | Vestra cache on ms | On/off | MoviePy get_frame ms | Exact RGBA equality |
|---|---:|---:|---:|---:|---|
| sequential | 44.667 | 53.311 | 1.19x | 38.770 | passed |
| repeated_frame | 8.384 | 8.698 | 1.04x | 7.100 | passed |
| local_reuse | 132.655 | 6.331 | 0.05x | 3352.130 | passed |
| random_seeking | 168.501 | 170.203 | 1.01x | 1036.385 | passed |
| eviction | 18.149 | 21.128 | 1.16x | 159.907 | passed |
| repeated_placements | 45.425 | 56.143 | 1.24x | 119.878 | passed |

The cache harness passed exact RGBA equality for every request. Cache helps strongly for `local_reuse` (132.655 ms → 6.331 ms, 0.05x) but adds overhead for sequential, repeated-frame, eviction, and repeated-placement sequences; that is expected for cursor-retention/request patterns and is not evidence of a broken cache. Public API repeated-placement output validation also passed.

| Sustained engine | Duration/frames | Wall s | FPS | Peak tree RSS MiB | CPU % |
|---|---:|---:|---:|---:|---:|
| vestra-cpu | 3600 s / 108000 | 326.218 | 331.07 | 544.1 | 798.1 |
| hardware-wgpu | 3600 s / 108000 | 165.422 | 652.88 | 667.4 | 394.9 |
| moviepy-cpu | 3600 s / 108000 | 1309.201 | 82.49 | 329.7 | 190.6 |

The sustained test rendered the same 640x360/30 fps two-placement timeline for one requested hour. Vestra CPU completed in 326.218 s (331.08 FPS), WGPU in 165.422 s (652.94 FPS), and MoviePy CPU in 1309.201 s (82.49 FPS). RSS stayed bounded in the sampled series; this is not a PSS or VRAM measurement.

## Targeted microbenchmarks

| Control | Result |
|---|---|
| preparation/pipeline-floor 1280x720 | 55.93 ms |
| preparation/animated-transform 1280x720 | 1.41 ms |
| preparation/mixed-layers-10 1920x1080 | 0.79 ms |
| effect scaling/effects-0 (30 frames, CPU) | 11.12 ms/frame, 89.93 FPS |
| effect scaling/effects-1 (30 frames, CPU) | 23.61 ms/frame, 42.35 FPS |
| effect scaling/effects-3 (30 frames, CPU) | 44.46 ms/frame, 22.49 FPS |
| effect scaling/effects-5 (30 frames, CPU) | 66.78 ms/frame, 14.97 FPS |
| random access/static-image frame 0 cold | 72.14 ms |
| random access/static-image frame 99 warm | 2.12 ms |
| random access/animated-transform frame 0 cold | 56.44 ms |
| random access/animated-transform frame 99 warm | 212.01 ms |

All three gated internal benchmark tests (`preparation_matrix`, `effect_scaling_matrix`, `random_access_matrix`) passed. The effect-scaling run is a focused CPU control, not a MoviePy comparison.

## Historical weaknesses and optimization priorities

| Rank | Measured weakness | Evidence | Impact | Effort | Confidence | Concrete next step |
|---:|---|---|---|---|---|---|
| 1 | CPU full-frame effect kernels | 1080p CPU `contrast_dither_video` 7.654 s/11.76 FPS, ASCII 6.345 s/14.18 FPS; corresponding WGPU 1.78/1.73 s (~50 FPS). Internal effect scaling rises 11.12 → 66.78 ms/frame from 0→5 effects. | High for CPU effect-heavy work | Medium | High | SIMD/tile the existing CPU kernels (rayon-friendly contiguous tiles), eliminate intermediate full-frame copies, and use a profiler to confirm cache/memory bandwidth before changing algorithms. |
| 2 | CPU compositing / eligibility path | `row-copy-1080p` opaque fast-path 4.629 s vs ineligible opacity 10.644 s (2.30x); WGPU is ~4.6–4.9 s for both, showing CPU path is the gap. | High for layered CPU video | Low–medium | High | Extend the existing opaque row-copy eligibility to more provably-opaque transforms and avoid generic blend surfaces when alpha is known. |
| 3 | CPU end-to-end utilization and memory | Matched MoviePy comparison: Vestra CPU is 4.92x faster editing and 2.36x faster layered, but 467–629 MiB RSS versus MoviePy 224–398 MiB; sustained Vestra CPU 543 MiB RSS vs MoviePy 330 MiB. | Medium (throughput already better; memory matters for concurrency) | Medium | High | Profile allocator/copy sites and worker scratch retention; bound per-worker scratch/cache budgets and measure PSS/VRAM separately before tuning. |
| 4 | GPU readback/row repack at 4K | Correct 4K WGPU dither smoke: 14.93 FPS, 398 MiB staging, 857 ms row repack, 1.95 GB texture uploads for 90 frames. | Medium for high-resolution WGPU | Medium | High | Keep frames GPU-resident through encoding where possible, batch/parallelize readback, and reduce row repacking/copies; validate hardware parity after each change. |
| 5 | Decoder/cache behavior on non-reuse access | Cache-on is slower for sequential (53.311 vs 44.667 ms), repeated-frame (8.698 vs 8.384 ms), eviction (21.128 vs 18.149 ms), and repeated-placement (56.143 vs 45.425 ms); random seeking is neutral (170.203 vs 168.501 ms). | Medium for seek-heavy edits; low for reuse-heavy edits | Medium | Medium | Instrument decoder seeks/conversions and cache hit/eviction decisions; tune budget/admission by access pattern instead of increasing the budget blindly. |
| 6 | Worker scaling evidence is incomplete | CPU sustained uses ~8.0 observed cores; no public worker-count control was benchmarked in this run, and the native worker sweep requires private fixtures. | Unknown | Medium | Low–medium | Run the existing `native_workers_1/2/4/8` ignored sweep with the documented external FFmpeg references before changing scheduling. |

These priorities describe the recorded baseline; remeasure the current revision
before choosing optimization work. They separate measured bottlenecks from
hypotheses. No claim is made that a particular third-party library will improve
results without profiling; plausible candidates for follow-up are
`rayon`-aligned SIMD kernels already present in the Rust ecosystem,
`ffmpeg-next`/FFmpeg hardware frames for reducing copies, and `wgpu`
staging/readback APIs. Adding a library is not justified by these measurements alone.

## Limitations and unverified comparisons

- MoviePy has no equivalent implementation of Vestra’s stylized effects, masks/mattes/particles suite scenarios, or WGPU backend. Those rows are Vestra-versus-Vestra-backend measurements; they are not MoviePy ratios.
- RSS is available; PSS and GPU VRAM are unavailable in the existing portable harness. RSS can double-count shared pages and sampled peaks are lower bounds.
- The portable cache harness compares public MoviePy `get_frame` latency with native Vestra access, not identical internal cache implementations; MoviePy cache counters are unavailable.
- Short suite inputs use deterministic generated/FFV1 footage and the MoviePy comparison uses three prepared local clips. They do not represent every codec, disk state, audio graph, or camera source. The versioned smoke/subset suite names were not rerun once their unique workloads were covered by the canonical, community, and full effect suites.
- No worker-count sweep was run because the documented native-worker benchmark requires private external FFmpeg reference fixtures. Treat worker-scaling conclusions as unverified.
- CPU and WGPU stages overlap; summed `frame_sum`/stage counters are diagnostic, not additive wall-time components.
- The first standalone 4K WGPU invocation omitted `VESTRA_BENCH_BACKEND=wgpu` and therefore selected CPU; it was discarded and rerun with the explicit backend. The reported 4K WGPU numbers are from the corrected run.

## Raw evidence locations

- Full collector: `target/benchmark-results/full-collection/vestra-bench-linux-20261031T130534Z-180e48c2/` and its ZIP archive.
- Additional suite matrices: `target/benchmark-results/complete-suites/cpu/` and `target/benchmark-results/complete-suites/hardware-wgpu/`.
- 4K logs: `target/benchmark-results/complete-suites/4k-cpu-dither.log` and `4k-wgpu-dither.log`.
- Internal controls: `target/benchmark-results/render-preparation.json`, `render-effect-scaling.json`, `render-random-access.json`.
