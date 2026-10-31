# Benchmarks

The focused `row-copy-1080p` suite measures ten seconds of deterministic H.264
with two source-sized video presentations per frame, including an integer-clipped
overlay. `opaque_video` uses unit opacity; `ineligible_video` uses 0.9. Run with
`python scripts/benchmark.py run --suite row-copy-1080p --backend cpu --output target/benchmark-results/row-copy`.
It uses one warmup and three one-shot encoded renders per workload. In this
video-only scene `cpu_opaque_copy_fast_path_hits` counts row copies (there are
no static cached layers); expect 600 eligible hits and zero ineligible hits.
Use the same benchmark harness on a Phase 1 checkout for incremental comparison.
The corrected Phase 2 measurements and their limitations are recorded in
[PR #90](https://github.com/evgen2571/vestra/pull/90); filtered-reference timings
do not measure incremental improvement over Phase 1 production sampling.

The ignored CPU `opaque_row_copy_benchmark` compares Phase 1 production sampling
with row-copy/fallback at 720p, 1080p and 4K after exact pixel checks. Run in
release mode with `VESTRA_RASTER_ROW_COPY_BENCH_OUTPUT` set to a JSON output path.
`opacity_scan_benchmark` similarly measures alpha scans at 1080p/4K using
`VESTRA_OPACITY_BENCH_OUTPUT`. These focused timings exclude decoding/encoding.

The ignored media `video_access_benchmark_matrix` covers 30-second 1080p H.264
(GOP 60), 60-second H.264 (GOP 300), and 30-second VFR H.264, all with B-frames.
Set `VESTRA_VIDEO_ACCESS_BENCH_OUTPUT` to a fresh JSON path and run
`cargo test --release -p vestra-media video_access_benchmark_matrix -- --ignored --nocapture --test-threads=1`.
Baseline and candidate output files in the same directory reuse identical
fixtures and external FFmpeg RGBA/ffprobe PTS references under `media/` (about
9.1 GiB). One oracle/warmup pass precedes three timing passes at zero and 64 MiB
cache budgets. Frame-request timings exclude open, teardown and oracle reads;
open time and decoder/cache/memory counters are separate. Select one fixture
with `VESTRA_VIDEO_ACCESS_BENCH_FIXTURE=typical-1080p|long-gop|vfr` if needed.
After an oracle pass, `VESTRA_VIDEO_ACCESS_BENCH_MODE=timing` and
`VESTRA_VIDEO_ACCESS_BENCH_PATTERN=sequential` isolate repeated sequential timing
without rereading the full RGBA reference; discarded warmups are retained.
Preserve executables from isolated builds: a shared Cargo target directory can
reuse the other checkout's newer artifacts. Check revision and actual fast-path
or seek behavior before accepting a comparison.

For one-command CPU, hardware WGPU and MoviePy collection with a shareable ZIP,
see [Collect benchmark results](COLLECTING.md). Cache and sustained profiles are
explicitly opt-in; the established local baseline workflow below is unchanged.

The suite definition is in `suites.json`; representative projects are in
`projects/`. The runner, Rust benchmark and comparison tests are maintained
with the engine. Generated results belong under the ignored
`target/benchmark-results/` directory.

Capture a baseline on the machine where you will measure the candidate. The
repository keeps suite definitions, projects and comparison tooling; personal
machine captures are not distributed as performance targets.

```bash
just benchmark-smoke target/benchmark-results/smoke
just benchmark target/benchmark-results/before
# Make one change and leave sources unchanged for the complete suite.
just benchmark target/benchmark-results/after
just benchmark-compare target/benchmark-results/before/suite.json target/benchmark-results/after/suite.json
```

Use a new directory for every run. The canonical CPU suite uses 1280×720,
one warmup and five samples per workload. Hardware WGPU needs a separate
baseline and confirmed hardware selection for every sample. Smoke verifies
execution and is not performance evidence.

The comparison rejects incompatible workload identities, definitions,
environments and backend selections. Preserve the original local baseline;
create a new named baseline when workloads or the environment change.

Reports include machine, toolchain, revision and executable metadata to make
local comparisons meaningful. Inspect and sanitize them before sharing, and
avoid committing home paths, environment dumps or workstation descriptions.
See [performance methodology](../docs/development/performance.md).

The ignored CPU `cpu_video_worker_count_benchmark` uses two presentations of
one source. It compares the ordinary one-worker cursor path with bounded
four-frame temporal batches for multi-worker renders, and prints wall time,
decoder sessions, requests, native decodes, cache hits/misses and decode time.
Run it in release mode with:

```bash
cargo test --release -p vestra-render-cpu cpu_video_worker_count_benchmark \
  -- --ignored --nocapture --test-threads=1
```

The focused color/dither suite measures a single full-frame moving FFV1 video
at 1920×1080, 30 fps and 90 frames. It records three separate workloads:
`stylization_baseline` (no effects), `palette_video` (smooth palette mapping),
and `dither_video` (fine Bayer8 dithering). All three use the same deterministic
`testsrc2` input; no titles, shapes, audio or additional layers are included.
Palette and dither color phase has an explicit three-second period. Media
creation is outside measured render intervals. The canonical ten-scenario
suite retains its settings and workload order.

```bash
python scripts/benchmark.py run --suite stylization-smoke --backend cpu --output target/benchmark-results/stylization-smoke
python scripts/benchmark.py run --suite stylization-1080p --backend cpu --output target/benchmark-results/stylization-before
# Repeat after a source-stable change with the same backend and machine.
python scripts/benchmark.py run --suite stylization-1080p --backend cpu --output target/benchmark-results/stylization-after
just benchmark-compare target/benchmark-results/stylization-before/suite.json target/benchmark-results/stylization-after/suite.json
```

The Full HD suite uses one warmup and three measured samples per scenario;
`stylization-smoke` uses 128×72, no warmup and one sample. For hardware results,
select `--backend hardware-wgpu` after adapter discovery and keep a separate
baseline. The existing runner rejects CPU fallback and software adapters.
The no-effects control reports its actual execution path, so use the recorded
frame/decode/encode stages to interpret overhead rather than attributing all
wall-time differences to shader execution.

The complete suite uses that same footage and adds the canonical ASCII and halftone examples,
horizontal and vertical bounded sorting, and seeded periodic CRT workloads. Effect settings come from the corresponding
`examples/effects/` JSON, with procedural periods set to three seconds.
The original three-workload suites retain their definitions for comparisons.

```bash
uv run --no-project python scripts/benchmark.py run --suite stylized-effects-smoke --backend cpu --output target/benchmark-results/effects-smoke
uv run --no-project python scripts/benchmark.py run --suite stylized-effects-1080p --backend cpu --output target/benchmark-results/effects-cpu
VESTRA_WGPU_BACKEND=gl uv run --no-project python scripts/benchmark.py run --suite stylized-effects-1080p --backend hardware-wgpu --output target/benchmark-results/effects-hardware
```

Both complete suites contain eight workloads; resolution, warmups and samples
match their three-workload counterparts. ASCII preparation includes its cached
bundled atlas. All samples retain preparation, decode, frame, encoding and
resource measurements. Keep sources unchanged during each suite and run
workloads serially, without concurrent compilation or other benchmarks.

For a 4K resource/correctness smoke check, use the built benchmark directly
with `VESTRA_BENCH_WIDTH=3840`, `VESTRA_BENCH_HEIGHT=2160`,
`VESTRA_BENCH_SCENARIO=dither_video`, `VESTRA_BENCH_WARMUPS=0`, and
`VESTRA_BENCH_SAMPLES=1`. This is not the versioned 1080p baseline.

The `dither-patterns-smoke` and `dither-patterns-1080p` suites compare the same
FFV1 control, legacy Bayer8, and stationary blue-noise dithering (`seed=37`).
The 1080p suite uses one warmup and three measured 90-frame samples per workload.
The original suites retain their workload lists so prior captures remain usable.
Run these suites with the existing runner and either `cpu` or `hardware-wgpu`.

The `chromatic-dither-smoke` and `chromatic-dither-1080p` suites compare the
FFV1 control and blue noise in luminance, RGB and hue-aware modes, using the same
editable four-color palette, seed, animation and sample settings. Their workload
lists are separate so historical pattern and effect suites retain their meaning.

The two-workload `channel-dither-smoke`/`channel-dither-1080p` suites compare
luminance blue noise with independent four-level RGB channels (64 output colors).
Both use the same generated FFV1 input, stationary blue-noise seed and sample
settings. Channel mode ignores palette phase; the luminance mode retains its
existing three-second palette cycle. The source remains identical.

The three-workload `perceptual-dither-smoke`/`perceptual-dither-1080p` suites
compare luminance, RGB and Oklab blue noise over that same input, palette, seed
and procedural phase. They use the usual smoke/1080p warmup and sample counts.

Benchmark samples explicitly include the selected adapter and Vestra's shared
`performance_class`, even though ordinary SDK result JSON omits adapter data.
Hardware verification uses this conservative classification; in particular,
WSL Mesa GL can report `device_type=other` for a D3D12-backed NVIDIA GPU.
Software, virtual and unknown classifications remain ineligible for hardware
suites. Raw device metadata is retained alongside the classification.

`tonal-stops-smoke` and `tonal-stops-1080p` compare the unchanged uniform
blue-noise scenario with `nonuniform_dither_video`, using the same four colors
at positions `(0, 0.18, 0.55, 1)`. The uneven-stop path searches at most 15
intervals per pixel. Both scenarios retain the existing source, periodic palette
animation, encoder and frame count.

`palette-interpolation-smoke` and `palette-interpolation-1080p` compare RGB and
Oklab gradient mapping with the existing periodic four-color palette video.
Both use identical decode/encode settings; end-to-end results include output
encoding cost and do not isolate inverse color-conversion shader cost.


`input-tone-{smoke,1080p}` compares the same seeded blue-noise video with neutral
input and with `input_exposure=0.5`, `input_gamma=1.5`. The active case includes
its full-resolution `ColorAdjust` pass and retained source. These end-to-end
measurements include decode/encode and output-dependent encoding cost.

`input-detail-{smoke,1080p}` compares that tone case with fine detail
(`input_detail=1.5`, radius `1`) and broad local contrast (`0.5`, radius `8`).
The latter two include three additional full-resolution preparation passes.
The 1080p suite uses one warmup and three measured samples per scenario; compare
sample ranges and actual adapter metadata before drawing performance conclusions.

The [MoviePy v2 comparison](comparisons/moviepy/README.md) adds three matched
local-footage workloads with CPU/hardware runs, process-tree measurements and
decoded-output validation through `scripts/benchmark.py moviepy`.
