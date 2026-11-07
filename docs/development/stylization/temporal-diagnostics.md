# Stylization temporal diagnostics

The facade's `gpu_stylization` parity tests save canonical projects, synthetic source
images, corresponding CPU/WGPU frames, parity metrics and adapter metadata to
`target/stylization/frames/<requested-api>/`. Temporal fixtures additionally save adjacent-frame
RGBA differences in `temporal.json`. Sources are original deterministic fixtures;
no downloaded media is needed.

WGPU-private resource-limit companions in `vestra-render-wgpu` write
`resources.json` for their corresponding cases in the same artifact tree.
Resource estimates are separate from facade pixel-parity artifacts.

## Run current diagnostic fixtures

Run from the repository root in the native contributor environment with an
actual adapter, selecting the API discovered by
[GPU validation](../gpu-validation.md). GL requires serialized tests:

```bash
VESTRA_WGPU_BACKEND=gl VESTRA_REQUIRE_WGPU=1 cargo test \
  -p vestra-render --test parity --all-features gpu_stylization_temporal \
  -- --test-threads=1 --nocapture

uv run --no-project --with 'Pillow>=10,<13' python \
  scripts/stylization-diagnostics.py
```

The diagnostic command reuses the [visual-regression skill](https://github.com/evgen2571/vestra/blob/develop/.agents/skills/visual-regression/SKILL.md)
comparison and contact-sheet tools. It writes `target/stylization/diagnostics/`
with per-timestamp CPU/WGPU difference PNGs, per-backend adjacent-frame
difference PNGs, two-column chronological contact sheets and `report.json`.
Use repeated `--fixture gl/NAME` to select fixtures, or
`--frames target/stylization/frames/gl --fixture NAME`. `--frames` and `--output`
accept alternative artifact directories. Requested-API directories identify the
requested API; actual identity is in `adapter.json`. Archive each run before
another run with the same API overwrites it. Follow [GPU validation](../gpu-validation.md)
for separate software and hardware evidence.

To check the diagnostic script itself:

```bash
uv run --no-project python -m unittest discover \
  -s tests -p test_stylization_diagnostics.py -v
```

## Interpret temporal differences

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

## Check full-resolution pixels and resources

Run full-resolution correctness separately:

```bash
VESTRA_WGPU_BACKEND=gl VESTRA_REQUIRE_WGPU=1 cargo test \
  -p vestra-render --test parity --all-features \
  gpu_stylization_all_families_1080p_and_4k_resource_validation \
  -- --ignored --test-threads=1 --nocapture

VESTRA_WGPU_BACKEND=gl VESTRA_REQUIRE_WGPU=1 cargo test \
  -p vestra-render-wgpu --lib gpu_stylization_ \
  -- --ignored --test-threads=1 --nocapture
```

The facade fixture renders ASCII, halftone, sorting and CRT at 1920×1080 and
3840×2160 and compares actual CPU/WGPU output. The WGPU companions retain
resource-limit checks for nonuniform stops, Oklab interpolation, input tone and
input detail, recording
persistent, working, effect, parameter and readback byte estimates. Estimates
exclude driver metadata and device texture padding. These are correctness/resource
checks; PNG saving and debug builds make their elapsed time unsuitable as a
performance benchmark.

To isolate CRT curvature at sub-byte transparent border coverage, run
`gpu_stylization_crt_1080p_and_4k_transparent_border_parity` with the same
ignored-test flags. This reuses the full-resolution fixtures and raw RGBA
comparison, including RGB values in pixels whose alpha rounds to zero.

## Compare three palettes

To regenerate the fine-pattern comparison, run this separate fixture with the
same adapter requirements as above:

```bash
VESTRA_WGPU_BACKEND=gl VESTRA_REQUIRE_WGPU=1 cargo test \
  -p vestra-render --test parity --all-features \
  gpu_stylization_fine_dither_preserves_scene_detail_with_three_distinct_palettes \
  -- --test-threads=1 --nocapture
```

Create a two-column sheet from
`{monochrome,ember,ocean}-{bayer8,blue_noise}/cpu-0.png` under
`target/stylization/frames/<requested-api>/`. Inspect native frames as well as
thumbnails; dense one-pixel patterns can alias in a reduced contact sheet. Use
the [dither regression checklist](dithered-palette-look.md#regression-checklist)
for the visual criteria.

<span id="advanced-control-visual-acceptance-matrix"></span>

## Historical freeze evidence

The [2026-10-10 release-freeze matrix](https://github.com/evgen2571/vestra/blob/fa0e95a1c896d01798f54e27d7e9bed3623734c7/docs/development/stylization/temporal-diagnostics.md#advanced-control-visual-acceptance-matrix)
retains the captured fixture names, visual observations and deferred controls.
Its statuses describe that capture, not verification of today's checkout or a
promise to implement the deferred controls. For a new change, use the current
procedures above, [effect reference](../../reference/effects.md) and
[support matrix](../../reference/feature-support.md).
