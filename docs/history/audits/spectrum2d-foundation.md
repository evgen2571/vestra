# Spectrum2D Foundation finalization audit

Status: Spectrum2D Foundation complete with minor notes. This is a final-state
audit for the accepted Spectrum2D Foundation. It records current evidence and
does not add another layout, preset, DSP path, or renderer feature.

## Verdict

The accepted Spectrum2D architecture and runtime guarantees remain intact. The
final cleanup adds retained 24-band and 48-band preparation measurements and a
public end-to-end pixel-contribution assertion. Rust validation and schema
checks pass. Full Python validation still has two unrelated existing contract
failures, and the repository-wide `ruff` and `ty` commands report existing
diagnostics. Those are recorded below and are not caused by this cleanup.

## Functional integration

`python-tests/test_authoring_render.py::test_public_spectrum2d_integration_project_exercises_audio_reactive_background_and_bloom`
builds two public Python projects. Both contain the same image background, RMS
audio-reactive scale, bundled tone, resolution, frame rate, and audio track.
The baseline has no Spectrum2D clip. The variant adds the `neon` Spectrum2D
clip. Both are prepared on the CPU and rendered at frame 10, one second into
the active bundled tone.

The test asserts that the complete RGBA buffers differ and that at least one
pixel in the configured Spectrum2D rectangle (`x=0.10..0.90`,
`y=0.70..1.00`) differs. The background animation is therefore present in both
renders and cannot explain the isolated region difference.

The same test retains the canonical JSON assertion: `neon` expands to explicit
Spectrum2D source fields and ordinary `glow` and `bloom` effects, with no
persisted preset identity. `examples/python/09_spectrum2d.py` remains the
corresponding runnable example.

## Prepared analysis and reuse

The staged prepared-state regression uses actual Spectrum2D Master BandEnergy
signals. It prepares once, renders random-access frames, renders a complete
video operation, renders another frame, and asserts that the analysis
invocation count remains `1`. Signal interning, logarithmic band mapping,
non-monotonic timestamp evaluation, CPU 1/2/4-worker byte equality, WGPU
multi-in-flight parameter isolation, and CPU/WGPU parity remain covered by the
existing core and renderer tests.

## Memory

`PreparedScalarSignal` retains one `Vec<f64>` per signal. The raw prepared
sample payload estimate is:

`duration_seconds × 100 samples/second × band_count × 8 bytes`

These are estimates of raw retained `f64` samples, not process RSS. They omit
allocator capacity, signal/vector metadata, PCM buffers, FFT temporary state,
and other analysis products.

| Duration | Bands | Raw sample bytes | Estimated size |
| ---: | ---: | ---: | ---: |
| 60 s | 24 | 1,152,000 | 1.10 MiB |
| 60 s | 48 | 2,304,000 | 2.20 MiB |
| 1 h | 24 | 69,120,000 | 65.92 MiB |
| 1 h | 48 | 138,240,000 | 131.84 MiB |
| 2 h maximum | 24 | 138,240,000 | 131.84 MiB |
| 2 h maximum | 48 | 276,480,000 | 263.67 MiB |

The two-hour maximum is the existing project limit. No second dense spectrum
matrix is created, and the 48-band cap is unchanged.

## Analysis benchmarks

The authoritative dedicated artifact is
`docs/audits/spectrum2d-foundation-benchmarks.json`. It retains the exact
configuration, command, environment, feature counts, FFT count, and measured
wall time for both primary Spectrum2D cases. The generic history remains in
`docs/audits/procedural-signal-audio-modulation-benchmarks.json` and keeps its
original 1/10/50-band and long-duration meaning.

Measured release preparation timings on synthetic streaming PCM:

| Duration | Bands | Requirements | FFT calls | Preparation wall time |
| ---: | ---: | ---: | ---: | ---: |
| 60 s | 24 | 26 | 11,992 | 381 ms |
| 60 s | 48 | 50 | 11,992 | 475 ms |

Exact reproducible commands:

```bash
VIDEO_EDITOR_ANALYSIS_BENCH_SECONDS=60 VIDEO_EDITOR_ANALYSIS_BENCH_BANDS=24 \
cargo test --release -p video-editor-media \
  audio_analysis::tests::master_analysis_benchmark -- \
  --exact --ignored --nocapture

VIDEO_EDITOR_ANALYSIS_BENCH_SECONDS=60 VIDEO_EDITOR_ANALYSIS_BENCH_BANDS=48 \
cargo test --release -p video-editor-media \
  audio_analysis::tests::master_analysis_benchmark -- \
  --exact --ignored --nocapture
```

## Renderer benchmarks

These are measured CPU renderer-only runs using synthetic, already-evaluated
band data. They do not measure end-to-end audio visualizer performance.

Command:

```bash
cargo test --release -p video-editor-render spectrum2d_cpu_benchmark \
  -- --ignored --nocapture
```

Configuration: 1920×1080, 24 frames, release build.

| Bands | Workers | Wall time | Effective FPS |
| ---: | ---: | ---: | ---: |
| 24 | 1 | 441.784 ms | 54.33 |
| 48 | 1 | 454.281 ms | 52.83 |
| 24 | auto (8) | 85.961 ms | 279.20 |
| 48 | auto (8) | 116.214 ms | 206.52 |

The same rows are retained under `renderer_only_measurements` in the
dedicated benchmark artifact.

## Benchmark environment

- OS: Linux, kernel `7.0.0-27-generic`, x86_64.
- CPU: Intel Core i7-8750H CPU @ 2.20GHz.
- Logical CPUs visible to the process: 12.
- CPU automatic renderer worker count: 8.
- Build mode: Cargo release profile for both benchmark harnesses.
- GPU/WGPU: unavailable. Adapter initialization returned
  `WGPU-ADAPTER-NOT-FOUND`, with message `WGPU adapter request returned no
  compatible adapter`.

No WGPU runtime benchmark is reported. Shader parsing, parameter-layout tests,
plan tests, and adapter-gated runtime tests still ran. The adapter-gated tests
reported the same explicit skip condition.

## CPU/WGPU parity and parameter audit

CPU tests cover zero-gap and nonzero-gap fractional bars, opacity, ordinary
effects, Bloom, and 1/2/4-worker byte equivalence. WGPU tests cover the same
Spectrum2D source/effect cases, multi-in-flight frame data isolation, and
adapter-independent packing. Runtime GPU comparisons were skipped on this host
because no compatible adapter exists.

`Spectrum2DParameters` remains 240 bytes. The dynamic uniform arena uses a
256-byte alignment, so the record stride remains 256 bytes.

## Schema, Python API, and presets

The schema generation and consistency commands pass:

```bash
python3 crates/video-editor-cli/tests/schema_validation.py
cargo run -q -p video-editor-cli -- generate-schema --output <temporary>/project.schema.json
cmp <temporary>/project.schema.json schemas/project.schema.json
```

The final checked schema remains unchanged and retains the Spectrum2D bounds:
band count 1..=48, positive frequencies, `max_hz <= 24000`, and
`0 <= bar_gap_ratio < 1`.

The focused Python authoring, preset, and public rendering tests passed: 30
passed and 1 deselected when running the Spectrum2D authoring/render selection.
The full suite result is recorded in Validation because it includes unrelated
failures.

`classic`, `dense`, and `neon` remain authoring-time expansions only. Canonical
JSON contains explicit Spectrum2D parameters and ordinary effects, never a
persisted preset name. The public API remains `Spectrum2DClip` and
`ProjectBuilder.add_spectrum2d_clip`.

## Validation commands and results

| Command | Status | Result |
| --- | --- | --- |
| `cargo fmt --check` | PASS | clean |
| `cargo check --workspace --all-features` | PASS | completed successfully |
| `cargo test --workspace --all-features` | PASS | 561 passed, 5 ignored |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | clean |
| `./scripts/check.sh` | PASS | Rust checks/tests and schema consistency passed |
| `CARGO_BUILD_JOBS=1 cargo test -j1 -p video-editor --lib render::engine::tests::staged_tests::prepared_audio_analysis_is_reused_across_random_access_and_video_operations -- --nocapture` | PASS | 1 passed |
| `cargo test -p video-editor-core spectrum -- --list` | PASS | 15 Spectrum2D tests listed |
| `cargo test -p video-editor-render spectrum2d -- --list` | PASS | 10 Spectrum2D tests listed |
| `cargo test -p video-editor-render wgpu:: -- --nocapture` | PASS | 95 tests passed; adapter-gated runtime cases skipped |
| `python3 crates/video-editor-cli/tests/schema_validation.py` plus generated-schema `cmp` | PASS | checked artifact unchanged |
| `.venv/bin/python -m pytest -q python-tests/test_authoring_spectrum2d.py python-tests/test_authoring_render.py -k spectrum2d` | PASS | 30 passed, 1 deselected |
| `.venv/bin/python -m pytest -q python-tests` | FAIL | 317 passed, 2 unrelated failures, 4 WGPU skips |
| `ruff check .` | FAIL | 95 existing repository diagnostics |
| `ty check` | FAIL | 452 existing repository diagnostics |
| `cargo test --release -p video-editor-media ... --exact --ignored --nocapture` | PASS | explicit 24/48 measurements retained above |
| `cargo test --release -p video-editor-render spectrum2d_cpu_benchmark -- --ignored --nocapture` | PASS | four CPU renderer rows retained above |

The two full-Python failures are
`test_encoder_failure_preserves_structured_render_context` and
`test_render_event_snapshots_keep_the_sdk_contract`. They reproduce in
isolation and concern existing render-stage/event expectations, not the changed
Spectrum2D integration test. They remain minor notes rather than being hidden
or changed as part of this focused cleanup.

## Architecture and compatibility

The runtime path remains Python/canonical JSON → `VisualSource::Spectrum2D` →
`CompiledVisualSource::Spectrum2D` → prepared Master BandEnergy signals →
`EvaluatedSource::Spectrum2D` → CPU or WGPU linear bars → ordinary effects →
opacity/blend/composition. Master audio remains required, preparation remains
single-use and random-access deterministic, and presets remain authoring-only.

Image, SolidColor, video/audio clips, effects, audio effects, Python APIs, JSON
projects, and CLI contracts remain covered by the passing compatibility suite.

## Scope and remaining issues

This cleanup did not implement new layouts, new presets, particles, 3D, a new
FFT/DSP architecture, renderer redesign, or GUI work.

Blockers: none for the Spectrum2D Foundation cleanup.

Minor notes: the host has no compatible WGPU adapter; two unrelated Python
contract tests fail; repository-wide `ruff` and `ty` are not clean.

Future opportunity: expand Spectrum2D layouts and styles in a separate phase.
