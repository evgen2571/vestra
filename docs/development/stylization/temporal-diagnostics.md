# Stylization temporal diagnostics

The renderer's `gpu_stylization` tests save canonical projects, synthetic source
images, corresponding CPU/WGPU frames, parity metrics, adapter metadata and resource byte estimates to
`target/stylization/frames/<requested-api>/`. Temporal fixtures additionally save adjacent-frame
RGBA differences in `temporal.json`. Sources are original deterministic fixtures;
no downloaded media is needed.

Run the focused tests with an actual adapter, selecting the API discovered by
[GPU validation](../gpu-validation.md). GL requires serialized tests:

```bash
VESTRA_WGPU_BACKEND=gl VESTRA_REQUIRE_WGPU=1 cargo test \
  -p vestra-render --lib --all-features gpu_stylization_temporal \
  -- --test-threads=1 --nocapture

uv run --no-project --with 'Pillow>=10,<13' python \
  scripts/stylization-diagnostics.py
```

The diagnostic command reuses the [visual-regression skill](../../../.agents/skills/visual-regression/SKILL.md)
comparison and contact-sheet tools. It writes `target/stylization/diagnostics/`
with per-timestamp CPU/WGPU difference PNGs, per-backend adjacent-frame
difference PNGs, two-column chronological contact sheets and `report.json`.
Use repeated `--fixture gl/NAME` to inspect selected hardware fixtures, or
`--frames target/stylization/frames/gl --fixture NAME`. Requested-API directories
keep software and hardware runs separate; actual adapter identity remains in
`adapter.json`. `--frames` and
`--output` accept alternative artifact directories. No runtime dependency is
added to Vestra. Test harness discovery with:

```bash
uv run --no-project python -m unittest discover \
  -s tests -p test_stylization_diagnostics.py -v
```

The fixtures distinguish intended motion from unwanted frame-state changes:

- `temporal-static-*` and `temporal-motion-*`: ASCII characters/geometric,
  all halftone color modes, both sort directions, and CRT. Stationary controls
  must be identical; CRT grain/jitter/flicker/rolling are disabled in these
  controls so expected procedural animation cannot hide unwanted changes.
  Moving scenes use an original detailed subject translated by whole pixels.
- `temporal-amount-*`: the same families with a keyframed source/effect mix
  from zero to full strength, including out-of-order revisit checks.
- `temporal-threshold-ascii`, `temporal-threshold-halftone-soft`, and
  `temporal-threshold-halftone-crisp`: gradual tonal changes at glyph/dot
  boundaries. Image dimensions that are not cell-size multiples include partial edge cells. Hard glyph
  selection and authored zero-softness dot coverage may intentionally step.
- `temporal-crt-period`: seeded, continuously animated analog controls
  around the procedural loop boundary, including random-access revisits.

- `temporal-stationary`: fixed gray subject and Bayer grid; adjacent rendered
  frames must be identical.
- `temporal-moving`: whole-pixel translated subject, including transparent
  borders; the shared interior retains the same composition-pixel Bayer grid.
  Differences at moving subject boundaries are expected.
- `temporal-threshold-sweep`: white subject slowly fades over opaque black
  before global dithering. Coverage changes monotonically at fixed Bayer
  thresholds. Crisp changes are intentional; smoothing would alter the style.
- `temporal-seam-gradient` and `temporal-seam-rainbow`: static detailed source
  with a two-second procedural palette period. Frames bracket the seam by one
  nanosecond and one millisecond; exact cycle endpoints repeat. Tests also
  render timestamps out of order and revisit them.

Inspect native PNGs for one-pixel patterns as well as contact sheets: thumbnail
resampling can introduce a visible moiré pattern. Metrics include raw RGBA,
including hidden RGB, and are diagnostic rather than a universal flicker gate.
Always compare CPU and WGPU motion separately, inspect threshold and seam
frames, and record the actual adapter. A GPU passing the static control does
not establish temporal quality for unrelated footage or every effect.

Run full-resolution correctness separately:

```bash
VESTRA_WGPU_BACKEND=gl VESTRA_REQUIRE_WGPU=1 cargo test \
  -p vestra-render --lib --all-features \
  gpu_stylization_all_families_1080p_and_4k_resource_validation \
  -- --ignored --test-threads=1 --nocapture
```

This explicit test renders every new family at 1920×1080 and 3840×2160,
compares actual CPU/WGPU output, and records persistent, working, effect,
parameter and readback byte estimates. Estimates exclude driver metadata and
device texture padding. The test is correctness/resource validation; PNG saving
and debug builds make its elapsed time unsuitable as a performance benchmark.
