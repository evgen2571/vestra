# Procedural signal and audio modulation audit

## Public surface

Python authoring exposes immutable Master `rms()`, `peak()`, and `band()`
signals. Signals support `gain`, `remap`, `clamp`, `envelope`, and
`response_curve`. Scalar targets accept ordered `replace`, `add`, and
`multiply` modifiers. Uniform scale writes the same signal specification to
both scale components.

The feature remains Master-only and scalar-only. It does not add beat, onset,
tempo, spectrum/vector, LFO, expression, persistent-cache, anchor, relational
black/white point, or integer-property modulation.

A deterministic executable reference lives in
`examples/python/08_audio_reactive.py`. It synthesizes a stepped low/high
frequency Master and drives uniform scale, Glow intensity, and Chromatic
Aberration while intentionally using `output_audio=False`.

## Execution boundaries

The compiler interns complete signal specifications and derives raw analysis
requirements. `video-editor-media` receives those raw requirements and produces
Master RMS, peak, and band-energy series. Core preparation applies envelope and
response-curve transforms, then frame evaluation samples immutable prepared
signals at absolute project time. CPU and WGPU receive only evaluated visual
values.

An empty requirement list skips analysis. `output.audio=false` disables muxing
only and does not remove authored Master audio from visual analysis.

Prepared-state regression coverage instruments analysis only in tests: one
analysis invocation is expected during `prepare`, and subsequent random-access
frames plus video operations must leave the invocation count unchanged.

## Source audit

The renderer search for `ScalarSignal`, `CompiledScalarProperty`,
`CompiledSignalTransform`, `AudioScalarFeature`, `BandEnergy`, `Envelope`, and
`ResponseCurve` has only test references. Production render modules contain no
signal semantics. The media search for generic transforms and property modifier
types has no matches. Its responsibility is raw DSP only.

`CompiledScalarProperty` keeps `Deref<Target = Track<f64>>` because compiler
normalization and metrics still use authored tracks directly. The dereference is
documented as semantically incomplete for runtime use. Production frame
evaluation calls `CompiledScalarProperty::evaluate`, which applies modifiers,
absolute signal time, and target constraints.

## Reference and parity coverage

The final staged reference test uses actual Master analysis to cover:

- 40–160 Hz BandEnergy -> Envelope/ResponseCurve -> uniform scale;
- RMS -> Glow intensity, including the target's 0..4 constraint;
- 2–12 kHz BandEnergy -> Chromatic Aberration amount.

Public SDK parity also prepares the same canonical audio-reactive project on CPU
and WGPU and compares selected frames under the existing parity tolerance. WGPU
remains adapter-gated: only adapter absence may skip the parity branch.

Earlier DSP coverage remains authoritative for production-master equivalence,
overlapping fades, source placement/padding, stereo power semantics, Parseval
normalization, amplitude-squared energy scaling, Nyquist edges, and shared STFT
work.

## Verification and performance

`audio_analysis::tests::master_analysis_benchmark` is ignored by default and
uses streaming synthetic PCM. Timing should always be collected from an
optimized build:

```bash
VIDEO_EDITOR_ANALYSIS_BENCH_SECONDS=60 VIDEO_EDITOR_ANALYSIS_BENCH_BANDS=1 \
  cargo test --release -p video-editor-media master_analysis_benchmark -- --ignored --nocapture
```

The final benchmark matrix records fixed-duration band scaling plus long-duration
one-band baselines to
`docs/audits/procedural-signal-audio-modulation-benchmarks.json`:

```bash
cargo test --release -p video-editor-media master_analysis_benchmark_matrix \
  -- --ignored --nocapture
```

Matrix rows are:

- 60 seconds / 1 band;
- 60 seconds / 10 bands;
- 60 seconds / 50 bands;
- 600 seconds / 1 band;
- 3600 seconds / 1 band.

Complete-signal fan-out has a separate ignored release benchmark. It analyzes
one raw 40–160 Hz BandEnergy series and then prepares 1, 10, and 50 distinct
ordered transform pipelines from that same raw series:

```bash
cargo test --release -p video-editor-media transformed_signal_scaling_benchmark \
  -- --ignored --nocapture
```

Every row asserts one Master FFmpeg decode, one raw analysis requirement, one
raw feature series, and the same duration-determined FFT call count. Only the
cheap complete-signal transform preparation is allowed to scale with fan-out.

The structural acceptance criteria are more portable than wall-clock time:
Master decode count remains one per preparation, FFT call count is duration
based rather than band-count based, RMS/Peak-only analysis performs no FFT, and
prepared reuse performs no second analysis pass.

The earlier 60-second RMS + peak + one-band development run reported 14,791 ms
and 11,992 FFT calls in a non-release test build. Treat that only as historical
debug evidence; the JSON produced by the release matrix is the performance
baseline to retain for the host that runs final verification.

Prepared state retains scalar feature/output series, not the Master PCM stream.
The analysis coordinator keeps only its bounded PCM/STFT windows and scratch
state while preparing each series.

## Final verification checklist

Run the repository's normal Rust, schema, and Python suites plus the targeted
reference tests before declaring the phase complete:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
python3 crates/video-editor-cli/tests/schema_validation.py
python3 -m pytest python-tests
```

Also run the release benchmark matrix above and retain the generated JSON. WGPU
hardware parity may skip only when the existing adapter-unavailable diagnostics
classify the host as lacking a compatible adapter.
