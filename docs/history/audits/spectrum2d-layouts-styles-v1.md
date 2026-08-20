# Spectrum2D Layouts & Styles v1 audit

## Verdict

Spectrum2D Layouts & Styles v1 is complete with minor notes. The accepted
feature set is implemented, documented, benchmarked, and covered by focused
CPU, WGPU, Python, schema, and determinism tests.

## Architecture

Master audio produces the existing prepared scalar `BandEnergy` signals.
Layouts and styles are presentation-only; presets expand at authoring time;
CPU and WGPU consume the same evaluated source fields; and ordinary effects,
opacity, blending, and composition remain downstream. No renderer stores
temporal Spectrum2D state or recognizes preset names.

## Backward compatibility

Omitted layout, gradient, and minimum-height fields retain linear/bottom/
forward, solid colour, and zero minimum height. Existing `classic`, `dense`,
and `neon` expansions remain stable. Old JSON, non-Spectrum sources, audio,
video, effects, CPU rendering, WGPU planning, and Python round trips are
covered by the workspace suites.

## WGPU parameter contract

`Spectrum2DParameters` is 256 bytes, 16-byte aligned, and uses a 256-byte
dynamic uniform stride. Flags occupy non-overlapping geometry/style bits;
alpha is packed only in the RGBA colour words. Tests cover all anchors,
directions, mappings, gradient bits, packed colours, all 48 amplitudes, and
zeroed unused slots.

## Linear semantics

Bottom, top, and center anchors support forward, reverse, and center-out
mapping. Center-out maps `[0.25, 0.50, 1.00]` to visual heights
`[1.00, 0.50, 0.25, 0.25, 0.50, 1.00]`, duplicating displayed bars only.

## Radial semantics

`0°` is up, `90°` right, `180°` down, and `270°` left; angles are clockwise
positive. Inner radius, partial/full sweep, forward/reverse mapping, and
outward/inward/both growth are covered. Start angles normalize in authored
`f64` space before CPU/WGPU `f32` conversion. The CPU visits only the layout
bounding box. Full-circle indexing remains half-open at the seam.

## Gradient/min-height semantics

Solid colour is transformed once per source render. Across-band gradients cache
up to 48 transformed analysis-band colours, including center-out mirrors.
Along-bar gradients interpolate in absolute bar space and include alpha.
Minimum height is applied to linear and radial amplitudes; zero amplitude with
zero radial minimum remains empty.

## Analysis invariance

Layout/style changes do not alter DSP requirements. Center-out duplicates
visual placement, not `BandEnergy` analysis signals: 24 analysis bands produce
48 visual bars.

## Random-access determinism

Existing random-access frame evaluation remains stateless. Expanded CPU tests
render radial layout, gradient, and Bloom/Glow cases with one, two, and four
workers and require byte-identical RGBA by frame number. WGPU multi-in-flight
tests submit distinct layout/style frames before flush and compare each result
to its corresponding CPU frame when an adapter is available.

## CPU/WGPU parity

Adapter-gated parity covers top/center anchors, forward/reverse/center-out
mapping, minimum height, across-band and along-bar gradients including alpha,
radial outward/inward/both directions, rotated and multi-turn arcs, full-circle
seams, and radial Spectrum2D with Bloom. Linear comparisons are byte-exact;
radial comparisons use the established narrow boundary rule.

## WGPU multi-in-flight

Parameter records are per submitted frame and preserve flags, angles, inner
radius, colours, gradients, and amplitudes across in-flight completion order.
The host result is `SKIPPED — WGPU-ADAPTER-NOT-FOUND`; the tests remain in the
suite and are not weakened.

## Public Python API

Typed linear/radial layout and gradient objects reject invalid runtime types.
Nested preset policy is replacement by whole `layout` or `gradient` object,
not arbitrary deep merge. New-field override, canonical JSON, native round
trip, schema, and public render smoke tests are present.

## Presets

The exact supported authoring list is `classic`, `dense`, `neon`, `mirror`,
`center_out`, `circle`, `neon_circle`, and `arc`. Preset identity is not
serialized.

## Schema

Generated schema artifacts and project validation retain compatibility with
old Spectrum2D JSON that omits the new fields; no unrelated schema changes are
introduced.

## CPU benchmarks

Release renderer-only benchmark: 1920×1080, 24 frames, deterministic dynamic
amplitudes, automatic worker count 8 on this host. Exact rows and metadata are
persisted in `spectrum2d-layouts-styles-v1-benchmarks.json`. Center-out records
24 analysis bands and 48 displayed bars. The exact `neon_circle` workload uses
its region, gap, inner radius, min height, across-band gradient, Glow, and
Bloom.

## Performance-regression comparison

| Workload | Foundation FPS | Pre-finalization FPS | Post-finalization FPS |
| --- | ---: | ---: | ---: |
| linear / 24 / 1 worker | 54.33 | 22.30 | 48.93 |
| linear / 24 / auto (8) | 279.20 | 110.45 | 213.76 |

The fast paths recover the obvious regression substantially; exact FPS is not
treated as a guarantee because measurements vary by host load.

## WGPU benchmark/skip

`SKIPPED — WGPU-ADAPTER-NOT-FOUND`; no WGPU value is fabricated.

## Benchmark environment

Linux 7.0.0-27-generic x86_64, Intel Core i7-8750H @ 2.20GHz, 12 logical CPUs,
automatic worker count 8. The command is
`cargo test --release -p video-editor-render spectrum2d_cpu_benchmark -- --ignored --nocapture`.

## Validation matrix

| Check | Outcome | Evidence |
| --- | --- | --- |
| `cargo fmt --check` | PASS | final run |
| `cargo check --workspace --all-features` | PASS | final run |
| `cargo test --workspace --all-features` | PASS | 209 render tests, 82 SDK, 147 core, 87 media, and workspace integration/doc tests |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | final run |
| `./scripts/check.sh` | FAIL — pre-existing FFmpeg test | `write_failure_includes_captured_stderr_after_the_process_exits` failed with an empty write error; no Spectrum2D failure |
| focused Spectrum2D Python tests | PASS | 50 passed |
| full `pytest python-tests` | FAIL — unrelated baseline | 336 passed, 4 skipped, 2 existing render-contract failures |
| `ruff check python python-tests` | FAIL — existing baseline | 93 existing style/type-fixture diagnostics |
| `ty check python/video_editor python-tests/test_typing.py` | FAIL — existing baseline | 66 existing diagnostics |
| schema generation | PASS | `cargo run -p video-editor-cli -- generate-schema` |
| schema validation | PASS | `Draft202012Validator.check_schema` |
| WGSL parsing | PASS | `wgpu::shader_tests::texture_shaders_parse_without_a_gpu_adapter` |
| WGPU runtime parity | SKIPPED | `WGPU-ADAPTER-NOT-FOUND` |

The full Rust workspace suite is green independently of the unrelated
canonical-script FFmpeg test. Python failures are reported rather than hidden.

## Known limitations

WGPU runtime and WGPU benchmark execution require a compatible adapter. Radial
boundary comparisons retain the existing narrow rule for trig edge pixels.

## Final acceptance

No new layout, style, preset, DSP feature, renderer abstraction, or unrelated
scope was added in this finalization pass. Spectrum2D Layouts & Styles v1 is
accepted with the documented baseline-tooling and adapter notes above.
