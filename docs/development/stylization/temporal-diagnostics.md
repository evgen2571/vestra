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

To isolate CRT curvature at sub-byte transparent border coverage, run
`gpu_stylization_crt_1080p_and_4k_transparent_border_parity` with the same
ignored-test flags. This reuses the full-resolution fixtures and raw RGBA
comparison, including RGB values in pixels whose alpha rounds to zero.

## Advanced-control visual acceptance matrix

The active plan's Milestones 5–7 remain in progress. This matrix separates
actual new-control evidence from pending implementation/acceptance.

| Control or mode | Fixture / rendered evidence | Status |
| --- | --- | --- |
| Bayer8 vs blue noise, three palettes | `gpu_stylization_fine_dither_preserves_scene_detail_with_three_distinct_palettes`: same gradient/texture/silhouette/line source; all stops visible; identical indices across palette hues; exact hardware parity | Inspected contact sheet |
| Blue-noise seeds, arbitrary time | `gpu_stylization_blue_noise_is_seeded_spatial_and_preserves_palette_detail`: seeds 0, 37, unsigned maximum; out-of-order revisit; visible highlights; exact hardware and software parity | Verified |
| Blue-noise tone coverage and alpha | CPU tile test: five grayscale levels, partial alpha and hidden transparent RGB | Verified; composed alpha/mask coverage still to extend |
| Pattern resolution | `gpu_stylization_1080p_and_4k_match_cpu_with_fine_patterns`: both patterns, CPU and hardware, exact output; saved resource estimates | Verified |
| Blue-noise moving footage and animated palette | Generated FFV1 showcase, ember CPU/ocean hardware short videos; periodic public showcase tests | Inspected sample frames; full temporal motion review pending |
| RGB/hue quantization | `gpu_stylization_chromatic_quantization_preserves_palette_colors_and_source_detail`: source/legacy/RGB/hue contact sheet; exact hardware GL and software Vulkan parity; dark-red literal distinguishes metrics | Inspected contact sheet and ember video sample; broader moving/mask acceptance pending |
| Channel-count quantization | `gpu_stylization_channel_levels_preserve_detail_and_match_cpu_exactly`: 2/4/8/256 levels, map/Bayer8/blue noise, exact GL/Vulkan parity and 256-level identity; explicit 1080p/4K extrema test | Inspected source/2/4/8 contact sheet; broader motion/mask acceptance pending |
| Perceptual quantization | Q10 Oklab: literal lightness/rounded-tie/partial-alpha cases; source/metric contact sheet; three-palette figure/highlight checks; exact GL/Vulkan parity and explicit 1080p/4K checks | Inspected contact sheet and moving-preview sample; broader motion/stack/mask acceptance pending |
| Uneven palette stops and interpolation spaces | Nonuniform literal ramp/blue-noise coverage, three-palette scene contrast and periodic out-of-order GL/Vulkan parity | Stops and Oklab ramps/animated preview inspected; broader motion/mask acceptance pending |
| Tone, local detail and independent analysis resolution | Fine texture, edges, partial borders and moving subjects required | Pending |
| Calibrated ASCII tone/color/coverage and glyph geometry | Well-exposed FFV1 footage with zero source mixing; custom glyph/font fixture required | Pending |
| Halftone circle/ellipse/line and screen response | Grayscale, edges, channel angles and printed appearance required | Advanced controls pending |
| Bounded sorting measures/run/region controls | Both directions, stable ties, threshold transitions and masks required | Advanced controls pending |
| CRT mask/scanline/bleed controls | Distinct 1080p masks, alpha-safe geometry and procedural loops required | Advanced controls pending |
| Editable advanced looks | Source side-by-side scenes, effect stacking and music-background loop required | Pending |

To reproduce the three-palette pattern comparison, run the fine-dither test
above with strict WGPU and create a two-column sheet from
`{monochrome,ember,ocean}-{bayer8,blue_noise}/cpu-0.png`. Artifacts are under
`target/stylization/frames/<requested-api>/`. Inspect native frames as well as
thumbnails; dense one-pixel patterns can alias in a reduced contact sheet.
