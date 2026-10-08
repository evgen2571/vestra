---
name: visual-regression
description: "Use when validating Vestra rendered-frame output, CPU/WGPU parity, visual effects against reference frames, color/alpha regressions, or shader quality."
---

# Compare rendered video frames

Read [testing](../../../docs/development/testing.md),
[GPU validation](../../../docs/development/gpu-validation.md) and the
[effect pipeline](../../../docs/development/architecture/effect-pipeline.md).

## Procedure

1. Create deterministic image/video fixtures with fixed resolution, source
   assets, project time, seeds, font resources and parameters. Prefer
   licensed/synthetic input.
2. Render **corresponding frames** with CPU and WGPU. Prefer direct frame APIs
   over lossy MP4 comparison; when needed, extract matching decoded frames
   with FFmpeg using consistent positions.
3. Record the actual backend, graphics API, adapter and classification.
   Software WGPU is not hardware GPU verification.
4. Run the comparison script for metrics and a visible difference PNG, then
   use the contact-sheet script for side-by-side human review.
5. Check edge cases: fully/partly transparent pixels, thin contours,
   gradients, masks/mattes, nested groups, 1080p/4K, and moving footage.
   Verify periodic effects at out-of-order timestamps separated by a cycle.
6. Keep generated assets under `target/`; don't commit third-party media or
   performance claims without provenance.

## CLI tools

Use an isolated **Pillow-only skill dependency**, not a new Vestra runtime
dependency. Run from the repository root:

```bash
uv run --no-project --with 'Pillow>=10,<13' python \
  .agents/skills/visual-regression/scripts/compare_frames.py \
  target/frames/cpu.png target/frames/wgpu.png \
  --output-dir target/visual-regression/example \
  --pixel-tolerance 2 --max-mae 3 --max-mismatch-fraction 0.05

uv run --no-project --with 'Pillow>=10,<13' python \
  .agents/skills/visual-regression/scripts/create_contact_sheet.py \
  target/frames/source.png target/frames/cpu.png target/frames/wgpu.png \
  --labels source cpu wgpu --columns 3 \
  --output target/visual-regression/example/contact.png
```

The comparator returns **raw RGBA** MAE, max channel error, PSNR and the
fraction of pixels where any channel exceeds the specified tolerance. Raw RGBA
deliberately includes hidden RGB differences in transparent pixels. The
amplified `difference.png` includes alpha differences, and `metrics.json`
contains machine-readable data. Identical images have `psnr_db: null`
(infinite/undefined).

Exit codes: **0** if thresholds pass or report-only; **1** if an explicitly
supplied threshold fails; **2** for invalid inputs, including different
dimensions. Example thresholds are illustrative, **not universal**.

## Self-tests

```bash
uv run --no-project --with 'Pillow>=10,<13' python -m unittest discover \
  -s .agents/skills/visual-regression/tests -v
```

Metrics cannot replace judgment of aesthetic quality. Inspect rendered
frames with multiple palettes and actual motion where applicable.
