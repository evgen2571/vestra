# Specialist benchmark reference

Pick a suite or probe for the part of the engine you're investigating.
Use the [benchmark overview](README.md) for the standard collection workflow.

- [Render suites](#render-suites) cover effects, dithering and the CPU video copy path.
- [Native probes](#native-probes) cover audio duration, row copies and decoder access.
- [Preview and static render](#preview-and-static-render) covers two direct benchmark controls.

Use the [native contributor environment](../CONTRIBUTING.md#2-set-up-a-checkout)
and fresh output paths. Compare matched runs on the same machine and backend.
Smoke runs check execution. [Measurement methodology](../docs/development/performance.md#measure-and-compare)

## Render suites

Run from the repository root. Replace the suite and output path for each run.

```bash
uv run --no-project python benchmarks/benchmark.py run \
  --suite stylization-1080p --backend cpu --output target/benchmark-results/stylization-cpu
```

| Suite | What it compares |
| --- | --- |
| `effects` | All unique effect and control workloads |
| `row-copy-1080p` | Opaque and translucent video overlays on CPU. Expect 600 row-copy hits for the opaque case and zero for the translucent case. |
| `stylization-1080p` | No effect, Palette Map and Bayer dither on the same valley footage |
| `stylized-effects-1080p` | The same controls plus ASCII, halftone, pixel sorting in both directions and CRT |
| `dither-patterns-1080p` | Bayer and blue-noise patterns with a no-effect control |
| `chromatic-dither-1080p` | No-effect control plus luminance, RGB and hue-aware matching |
| `channel-dither-1080p` | Luminance matching and four-level RGB channel quantization |
| `perceptual-dither-1080p` | Luminance, RGB and Oklab matching |
| `tonal-stops-1080p` | Uniform and uneven palette stops |
| `palette-interpolation-1080p` | RGB and Oklab gradient interpolation |
| `input-tone-1080p` · `input-detail-1080p` | Input exposure, gamma and local detail passes |

Stylization and dither suites have matching `-smoke` variants. See the suite
file for exact workloads, output settings and sample counts. For hardware runs,
verify the adapter first, use `--backend hardware-wgpu` and keep a separate
baseline. [Suite definitions](suites.json) · [Adapter validation](../docs/development/gpu-validation.md)

## Native probes

These probes measure narrower operations. Compare each probe with its own baseline.

<details>
<summary>Audio duration probing</summary>

```bash
mkdir -p target/benchmark-results/audio-probing
ffmpeg -v error -f lavfi -i 'sine=sample_rate=48000:duration=1800' -ac 2 \
  target/benchmark-results/audio-probing/long.flac
VESTRA_BENCH_AUDIO=target/benchmark-results/audio-probing/long.flac \
  cargo test --release -p vestra-media benchmark_audio_probing -- --ignored --nocapture
```

Compares ffprobe, native stream metadata and decoded sample counts on the same
input. Repeat with short and long WAV, FLAC, MP3 or AAC/M4A files. Generate
inputs before timing. A null duration means the stream metadata is missing.

</details>

<details>
<summary>CPU row copies and alpha scans</summary>

| Ignored `vestra-render-cpu` test | Report path variable |
| --- | --- |
| `opaque_row_copy_benchmark` | `VESTRA_RASTER_ROW_COPY_BENCH_OUTPUT` |
| `opacity_scan_benchmark` | `VESTRA_OPACITY_BENCH_OUTPUT` |

Set the corresponding variable to a fresh JSON path and run the test with
`cargo test --release -p vestra-render-cpu TEST -- --ignored --nocapture`.
The probes exclude decode and encoding. Use the production path in your
baseline to measure an incremental improvement. [Row-copy measurements](https://github.com/evgen2571/vestra/pull/90)

</details>

<details>
<summary>Video decoder access patterns</summary>

```bash
VESTRA_VIDEO_ACCESS_BENCH_OUTPUT=target/benchmark-results/video-access.json \
  cargo test --release -p vestra-media video_access_benchmark_matrix -- \
  --ignored --nocapture --test-threads=1
```

Reserve about 9.1 GiB for fixtures and reference frames. Keep baseline and
candidate reports in the same directory to reuse those inputs. Set
`VESTRA_VIDEO_ACCESS_BENCH_FIXTURE` to `typical-1080p`, `long-gop` or `vfr`
to run one case. After generating the reference, use
`VESTRA_VIDEO_ACCESS_BENCH_MODE=timing` and
`VESTRA_VIDEO_ACCESS_BENCH_PATTERN=sequential` for sequential timing only.

</details>

For stage timings, allocation, cache and worker measurements, see
[Performance probes](../docs/development/performance-probes.md).

## Preview and static render

<details>
<summary>Compare preview and full renders</summary>

Use the built `animation_effects` executable. Run the same scenario twice with
distinct report paths, setting `VESTRA_BENCH_PREVIEW=1` for the preview run.

```bash
VESTRA_BENCH_SCENARIO=combined VESTRA_BENCH_WIDTH=1280 VESTRA_BENCH_HEIGHT=720 \
VESTRA_BENCH_BACKEND=cpu VESTRA_BENCH_QUALITY=balanced VESTRA_BENCH_WARMUPS=1 \
VESTRA_BENCH_SAMPLES=3 VESTRA_BENCH_REPORT=target/benchmark-results/full.json \
VESTRA_BENCH_KEEP_OUTPUT=1 /absolute/path/to/animation_effects
```

Compare decoded frames at matching times and report the resulting dimensions,
frame rate, quality and timings. The presets produce different workloads, so
the strict regression comparator rejects this pair. Remove the retained media
after inspecting it.

</details>

The `static_colour` workload checks the static FFmpeg path. Expect
`static_visual_ffmpeg_fast_path_used=true`, one static template population and
zero raw frame pushes for 300 encoded frames.
