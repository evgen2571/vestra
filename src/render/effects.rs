use image::{GenericImage, Rgba, RgbaImage};

use crate::plan::EvaluatedEffect;

/// Low-level CPU work produced by one evaluated project effect.
///
/// The pass plan is deliberately independent of the project format.  It makes
/// resource requirements and ordering explicit, while leaving the renderer
/// free to use the same plan on a future GPU backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CpuEffectPass {
    Single,
    GaussianHorizontal { radius: f64 },
    GaussianVertical { radius: f64 },
    HighlightExtract { threshold: f64, colour: [u8; 4] },
    GlowComposite { intensity: f64 },
    UnsharpComposite { amount: f64 },
}

/// Expands one evaluated effect into the CPU passes it requires. Identity
/// effects return no passes, so callers can avoid allocating intermediates.
#[must_use]
pub(crate) fn effect_pass_plan(effect: &EvaluatedEffect) -> Vec<CpuEffectPass> {
    match effect {
        EvaluatedEffect::GaussianBlur { radius } if *radius <= 0.0 => Vec::new(),
        EvaluatedEffect::GaussianBlur { radius } if *radius > 0.0 => vec![
            CpuEffectPass::GaussianHorizontal { radius: *radius },
            CpuEffectPass::GaussianVertical { radius: *radius },
        ],
        EvaluatedEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } if *radius > 0.0 && *intensity > 0.0 => vec![
            CpuEffectPass::HighlightExtract {
                threshold: *threshold,
                colour: *colour,
            },
            CpuEffectPass::GaussianHorizontal { radius: *radius },
            CpuEffectPass::GaussianVertical { radius: *radius },
            CpuEffectPass::GlowComposite {
                intensity: *intensity,
            },
        ],
        EvaluatedEffect::Glow { .. } => Vec::new(),
        EvaluatedEffect::Sharpen { amount, radius } if *amount > 0.0 && *radius > 0.0 => vec![
            CpuEffectPass::GaussianHorizontal { radius: *radius },
            CpuEffectPass::GaussianVertical { radius: *radius },
            CpuEffectPass::UnsharpComposite { amount: *amount },
        ],
        EvaluatedEffect::Sharpen { .. } => Vec::new(),
        EvaluatedEffect::CameraShake { .. } => Vec::new(),
        _ => vec![CpuEffectPass::Single],
    }
}

/// Applies a separable Gaussian blur using premultiplied-alpha accumulation.
/// Pixels remain stored as straight RGBA, so the last pass safely converts the
/// accumulated premultiplied colour back to the renderer's storage format.
pub(crate) fn gaussian_blur(
    source: &RgbaImage,
    horizontal: &mut RgbaImage,
    target: &mut RgbaImage,
    radius: f64,
) {
    if radius <= 0.01 {
        target.copy_from(source, 0, 0).expect("matching surfaces");
        return;
    }
    let kernel = GaussianKernel::new(radius);
    convolve(source, horizontal, &kernel, true);
    convolve(horizontal, target, &kernel, false);
}

pub(crate) fn glow(
    source: &RgbaImage,
    horizontal: &mut RgbaImage,
    target: &mut RgbaImage,
    threshold: f64,
    radius: f64,
    intensity: f64,
    colour: [u8; 4],
) {
    if intensity <= 0.0 || radius <= 0.01 {
        target.copy_from(source, 0, 0).expect("matching surfaces");
        return;
    }
    let threshold = threshold.clamp(0.0, 1.0);
    for (x, y, pixel) in source.enumerate_pixels() {
        let luminance = (f64::from(pixel[0]) * 0.2126
            + f64::from(pixel[1]) * 0.7152
            + f64::from(pixel[2]) * 0.0722)
            / 255.0;
        let highlight = ((luminance - threshold) / (1.0 - threshold).max(0.000_1)).clamp(0.0, 1.0);
        let alpha = (f64::from(pixel[3]) / 255.0 * highlight * f64::from(colour[3]) / 255.0)
            .clamp(0.0, 1.0);
        target.put_pixel(
            x,
            y,
            Rgba([
                (f64::from(colour[0]) * highlight).round() as u8,
                (f64::from(colour[1]) * highlight).round() as u8,
                (f64::from(colour[2]) * highlight).round() as u8,
                (alpha * 255.0).round() as u8,
            ]),
        );
    }
    let kernel = GaussianKernel::new(radius);
    convolve(target, horizontal, &kernel, true);
    convolve(horizontal, target, &kernel, false);
    for (base, bloom) in source.pixels().zip(target.pixels_mut()) {
        for channel in 0..3 {
            bloom[channel] = (f64::from(base[channel]) + f64::from(bloom[channel]) * intensity)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        bloom[3] = base[3];
    }
}

pub(crate) fn sharpen(
    source: &RgbaImage,
    horizontal: &mut RgbaImage,
    target: &mut RgbaImage,
    amount: f64,
    radius: f64,
) {
    if amount <= 0.0 || radius <= 0.01 {
        target.copy_from(source, 0, 0).expect("matching surfaces");
        return;
    }
    gaussian_blur(source, horizontal, target, radius);
    for (base, blurred) in source.pixels().zip(target.pixels_mut()) {
        for channel in 0..3 {
            blurred[channel] = (f64::from(base[channel])
                + (f64::from(base[channel]) - f64::from(blurred[channel])) * amount)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        blurred[3] = base[3];
    }
}

struct GaussianKernel {
    weights: Vec<f64>,
    radius: i32,
}

impl GaussianKernel {
    fn new(radius: f64) -> Self {
        let radius = radius.clamp(0.0, 32.0);
        let support = radius.ceil().max(1.0) as i32;
        let sigma = (radius / 3.0).max(0.5);
        let mut weights = Vec::with_capacity((support * 2 + 1) as usize);
        let mut sum = 0.0;
        for offset in -support..=support {
            let weight = (-0.5 * (f64::from(offset) / sigma).powi(2)).exp();
            weights.push(weight);
            sum += weight;
        }
        for weight in &mut weights {
            *weight /= sum;
        }
        Self {
            weights,
            radius: support,
        }
    }
}

fn convolve(source: &RgbaImage, target: &mut RgbaImage, kernel: &GaussianKernel, horizontal: bool) {
    let width = source.width() as i32;
    let height = source.height() as i32;
    for y in 0..height {
        for x in 0..width {
            let mut premultiplied = [0.0; 3];
            let mut alpha = 0.0;
            for (index, offset) in (-kernel.radius..=kernel.radius).enumerate() {
                let (sample_x, sample_y) = if horizontal {
                    ((x + offset).clamp(0, width - 1), y)
                } else {
                    (x, (y + offset).clamp(0, height - 1))
                };
                let pixel = source.get_pixel(sample_x as u32, sample_y as u32);
                let weight = kernel.weights[index];
                let sample_alpha = f64::from(pixel[3]) / 255.0;
                alpha += sample_alpha * weight;
                for channel in 0..3 {
                    premultiplied[channel] +=
                        f64::from(pixel[channel]) / 255.0 * sample_alpha * weight;
                }
            }
            let rgb = if alpha <= 0.000_000_1 {
                [0; 3]
            } else {
                premultiplied.map(|value| (value / alpha * 255.0).round().clamp(0.0, 255.0) as u8)
            };
            target.put_pixel(
                x as u32,
                y as u32,
                Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_effects_expand_into_explicit_ordered_passes() {
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Glow {
                threshold: 0.6,
                radius: 3.0,
                intensity: 0.75,
                colour: [255, 128, 64, 255],
            }),
            vec![
                CpuEffectPass::HighlightExtract {
                    threshold: 0.6,
                    colour: [255, 128, 64, 255],
                },
                CpuEffectPass::GaussianHorizontal { radius: 3.0 },
                CpuEffectPass::GaussianVertical { radius: 3.0 },
                CpuEffectPass::GlowComposite { intensity: 0.75 },
            ]
        );
        assert_eq!(
            effect_pass_plan(&EvaluatedEffect::Sharpen {
                amount: 0.0,
                radius: 2.0,
            }),
            Vec::new()
        );
    }

    #[test]
    fn gaussian_spreads_symmetrically_without_transparent_colour_halos() {
        let mut source = RgbaImage::from_pixel(7, 7, Rgba([255, 0, 0, 0]));
        source.put_pixel(3, 3, Rgba([255, 255, 255, 255]));
        let mut horizontal = RgbaImage::new(7, 7);
        let mut target = RgbaImage::new(7, 7);
        gaussian_blur(&source, &mut horizontal, &mut target, 2.0);
        assert_eq!(target.get_pixel(2, 3), target.get_pixel(4, 3));
        assert_eq!(target.get_pixel(3, 2), target.get_pixel(3, 4));
        assert!(target.get_pixel(2, 3)[3] > 0);
        assert_eq!(target.get_pixel(0, 0)[0], 0);
    }

    #[test]
    fn glow_spreads_on_both_axes_and_preserves_the_source() {
        let mut source = RgbaImage::new(7, 7);
        source.put_pixel(3, 3, Rgba([255, 255, 255, 255]));
        let mut horizontal = RgbaImage::new(7, 7);
        let mut target = RgbaImage::new(7, 7);
        glow(
            &source,
            &mut horizontal,
            &mut target,
            0.5,
            2.0,
            1.0,
            [255, 0, 0, 255],
        );
        assert_eq!(target.get_pixel(3, 3)[0], 255);
        assert!(target.get_pixel(2, 3)[0] > 0);
        assert!(target.get_pixel(3, 2)[0] > 0);
    }
}
