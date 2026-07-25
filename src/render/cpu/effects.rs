use std::cell::RefCell;

use image::{GenericImage, Rgba, RgbaImage};

use crate::plan::EvaluatedEffect;

use super::surfaces::EffectSurfacePool;

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

/// The largest built-in chain (glow) has four passes. Keeping this on the
/// stack avoids per-frame heap allocation while still preserving pass order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CpuEffectPassPlan {
    passes: [CpuEffectPass; 4],
    len: usize,
}

impl CpuEffectPassPlan {
    fn new(passes: &[CpuEffectPass]) -> Self {
        debug_assert!(passes.len() <= 4);
        let mut planned = [CpuEffectPass::Single; 4];
        planned[..passes.len()].copy_from_slice(passes);
        Self {
            passes: planned,
            len: passes.len(),
        }
    }

    #[must_use]
    pub(crate) fn is_empty(self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub(crate) fn as_slice(&self) -> &[CpuEffectPass] {
        &self.passes[..self.len]
    }
}

/// Expands one evaluated effect into the CPU passes it requires. Identity
/// effects return no passes, so callers can avoid allocating intermediates.
#[must_use]
pub(crate) fn effect_pass_plan(effect: &EvaluatedEffect) -> CpuEffectPassPlan {
    if effect.is_identity() {
        return CpuEffectPassPlan::new(&[]);
    }
    match effect {
        EvaluatedEffect::GaussianBlur { radius } => CpuEffectPassPlan::new(&[
            CpuEffectPass::GaussianHorizontal { radius: *radius },
            CpuEffectPass::GaussianVertical { radius: *radius },
        ]),
        EvaluatedEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } => CpuEffectPassPlan::new(&[
            CpuEffectPass::HighlightExtract {
                threshold: *threshold,
                colour: *colour,
            },
            CpuEffectPass::GaussianHorizontal { radius: *radius },
            CpuEffectPass::GaussianVertical { radius: *radius },
            CpuEffectPass::GlowComposite {
                intensity: *intensity,
            },
        ]),
        EvaluatedEffect::Sharpen { amount, radius } => CpuEffectPassPlan::new(&[
            CpuEffectPass::GaussianHorizontal { radius: *radius },
            CpuEffectPass::GaussianVertical { radius: *radius },
            CpuEffectPass::UnsharpComposite { amount: *amount },
        ]),
        _ => CpuEffectPassPlan::new(&[CpuEffectPass::Single]),
    }
}

/// Executes the logical CPU pass plan against reusable ping-pong surfaces.
pub(super) fn apply_chain(surfaces: &mut EffectSurfacePool, effects: &[EvaluatedEffect]) {
    for effect in effects {
        if effect_pass_plan(effect).is_empty() {
            continue;
        }
        surfaces.run(
            |source, target, horizontal| match effect_pass_plan(effect).as_slice() {
                [
                    CpuEffectPass::GaussianHorizontal { radius },
                    CpuEffectPass::GaussianVertical { .. },
                ] => gaussian_blur(source, horizontal, target, *radius),
                [
                    CpuEffectPass::HighlightExtract { threshold, colour },
                    CpuEffectPass::GaussianHorizontal { radius },
                    CpuEffectPass::GaussianVertical { .. },
                    CpuEffectPass::GlowComposite { intensity },
                ] => glow(
                    source, horizontal, target, *threshold, *radius, *intensity, *colour,
                ),
                [
                    CpuEffectPass::GaussianHorizontal { radius },
                    CpuEffectPass::GaussianVertical { .. },
                    CpuEffectPass::UnsharpComposite { amount },
                ] => sharpen(source, horizontal, target, *amount, *radius),
                [CpuEffectPass::Single] => apply_single(source, target, effect),
                [] => unreachable!("identity effects are skipped before execution"),
                _ => unreachable!("effect pass plans must be complete"),
            },
        );
    }
}

pub(super) fn apply_to(
    surfaces: &mut EffectSurfacePool,
    destination: &mut RgbaImage,
    effects: &[EvaluatedEffect],
) {
    if effects
        .iter()
        .all(|effect| effect_pass_plan(effect).is_empty())
    {
        return;
    }
    surfaces.begin_from(destination);
    apply_chain(surfaces, effects);
    surfaces.copy_to(destination);
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
    with_gaussian_kernel(radius, |kernel| {
        convolve(source, horizontal, kernel, true);
        convolve(horizontal, target, kernel, false);
    });
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
                colour[0],
                colour[1],
                colour[2],
                (alpha * 255.0).round() as u8,
            ]),
        );
    }
    with_gaussian_kernel(radius, |kernel| {
        convolve(target, horizontal, kernel, true);
        convolve(horizontal, target, kernel, false);
    });
    for (base, bloom) in source.pixels().zip(target.pixels_mut()) {
        let base_alpha = f64::from(base[3]) / 255.0;
        let glow_alpha = (f64::from(bloom[3]) / 255.0 * intensity).clamp(0.0, 1.0);
        let alpha = base_alpha + glow_alpha * (1.0 - base_alpha);
        let rgb = if alpha <= 0.000_000_1 {
            [0; 3]
        } else {
            std::array::from_fn(|channel| {
                ((f64::from(base[channel]) / 255.0 * base_alpha
                    + f64::from(bloom[channel]) / 255.0 * glow_alpha)
                    / alpha
                    * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
        };
        *bloom = Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]);
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

/// Runs an effect that requires one source and destination surface.
pub(crate) fn apply_single(source: &RgbaImage, target: &mut RgbaImage, effect: &EvaluatedEffect) {
    match effect {
        EvaluatedEffect::Brightness { amount } => map_pixels(source, target, |mut pixel| {
            for channel in 0..3 {
                pixel[channel] = (f64::from(pixel[channel]) + amount * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            pixel
        }),
        EvaluatedEffect::Contrast { amount } => map_pixels(source, target, |mut pixel| {
            for channel in 0..3 {
                pixel[channel] = ((f64::from(pixel[channel]) - 128.0) * amount + 128.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            pixel
        }),
        EvaluatedEffect::Saturation { amount } => map_pixels(source, target, |mut pixel| {
            let l = f64::from(pixel[0]) * 0.2126
                + f64::from(pixel[1]) * 0.7152
                + f64::from(pixel[2]) * 0.0722;
            for channel in 0..3 {
                pixel[channel] = (l * (1.0 - amount) + f64::from(pixel[channel]) * amount)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            pixel
        }),
        EvaluatedEffect::Tint { colour, amount } => map_pixels(source, target, |mut pixel| {
            let amount = amount.clamp(0.0, 1.0);
            for channel in 0..3 {
                pixel[channel] = (f64::from(pixel[channel]) * (1.0 - amount)
                    + f64::from(colour[channel]) * amount)
                    .round() as u8;
            }
            pixel
        }),
        EvaluatedEffect::GaussianBlur { .. }
        | EvaluatedEffect::Glow { .. }
        | EvaluatedEffect::Sharpen { .. } => {
            unreachable!("multi-pass effects are dispatched by the surface pool")
        }
        EvaluatedEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => blur(source, target, *radius, Some(*angle_degrees), None),
        EvaluatedEffect::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => super::zoom_blur::apply(source, target, *radius, *samples, *anchor, *direction),
        EvaluatedEffect::MotionBlur {
            radius,
            angle_degrees,
            samples,
            ..
        } => blur(
            source,
            target,
            *radius,
            Some(*angle_degrees),
            Some(*samples),
        ),
        EvaluatedEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => super::chromatic::apply(source, target, *amount, *angle_degrees),
        EvaluatedEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => super::vignette::apply(source, target, *amount, *radius, *softness, *colour),
        EvaluatedEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => super::colour_adjust::apply(
            source,
            target,
            *exposure,
            *gamma,
            *black_point,
            *white_point,
        ),
        EvaluatedEffect::CameraShake { .. } => {
            target.copy_from(source, 0, 0).expect("same dimensions")
        }
    }
}

fn map_pixels(
    source: &RgbaImage,
    target: &mut RgbaImage,
    mut map: impl FnMut(Rgba<u8>) -> Rgba<u8>,
) {
    for (x, y, pixel) in source.enumerate_pixels() {
        target.put_pixel(x, y, map(*pixel));
    }
}

pub(crate) fn blur(
    source: &RgbaImage,
    target: &mut RgbaImage,
    radius: f64,
    direction: Option<f64>,
    configured_samples: Option<u8>,
) {
    if radius <= 0.01 {
        target.copy_from(source, 0, 0).expect("same dimensions");
        return;
    }
    let radius = radius.clamp(0.0, 32.0);
    let samples =
        configured_samples.map_or_else(|| (radius.ceil() as i32 * 2 + 1).clamp(3, 33), i32::from);
    let (dx, dy) = direction.map_or((1.0, 0.0), |degrees| {
        let radians = degrees.to_radians();
        (radians.cos(), radians.sin())
    });
    for (x, y, _) in source.enumerate_pixels() {
        let mut premultiplied = [0.0; 3];
        let mut alpha = 0.0;
        for index in 0..samples {
            let offset = (f64::from(index) / f64::from(samples - 1) - 0.5) * 2.0 * radius;
            let (sx, sy) = if direction.is_some() {
                (f64::from(x) + dx * offset, f64::from(y) + dy * offset)
            } else {
                let angle = f64::from(index) / f64::from(samples) * std::f64::consts::TAU;
                (
                    f64::from(x) + angle.cos() * offset.abs(),
                    f64::from(y) + angle.sin() * offset.abs(),
                )
            };
            let pixel = super::raster::sample_edge(source, sx + 0.5, sy + 0.5);
            let sample_alpha = f64::from(pixel[3]) / 255.0;
            alpha += sample_alpha;
            for channel in 0..3 {
                premultiplied[channel] += f64::from(pixel[channel]) / 255.0 * sample_alpha;
            }
        }
        alpha /= f64::from(samples);
        let rgb = if alpha <= 0.000_000_1 {
            [0; 3]
        } else {
            premultiplied.map(|value| {
                (value / f64::from(samples) / alpha * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
        };
        target.put_pixel(
            x,
            y,
            Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]),
        );
    }
}

#[derive(Clone)]
struct GaussianKernel {
    weights: Vec<f64>,
    radius: i32,
}

thread_local! {
    static GAUSSIAN_KERNEL_CACHE: RefCell<Vec<(u16, GaussianKernel)>> = const { RefCell::new(Vec::new()) };
}

fn with_gaussian_kernel<T>(radius: f64, work: impl FnOnce(&GaussianKernel) -> T) -> T {
    let key = (radius.clamp(0.0, 32.0) * 4.0).round() as u16;
    GAUSSIAN_KERNEL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache
            .iter()
            .position(|(cached, _)| *cached == key)
            .unwrap_or_else(|| {
                if cache.len() == 16 {
                    cache.remove(0);
                }
                cache.push((key, GaussianKernel::new(f64::from(key) / 4.0)));
                cache.len() - 1
            });
        work(&cache[index].1)
    })
}

#[cfg(test)]
fn gaussian_kernel_cache_len() -> usize {
    GAUSSIAN_KERNEL_CACHE.with(|cache| cache.borrow().len())
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
            })
            .as_slice(),
            &[
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
            })
            .as_slice(),
            &[]
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
    fn transparent_glow_matches_the_pixel_golden_fixture() {
        let mut source = RgbaImage::new(3, 3);
        source.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let mut horizontal = RgbaImage::new(3, 3);
        let mut output = RgbaImage::new(3, 3);
        glow(
            &source,
            &mut horizontal,
            &mut output,
            0.0,
            1.0,
            1.0,
            [255, 64, 0, 255],
        );
        assert_eq!(
            output.as_raw(),
            &[
                255, 64, 0, 3, 255, 64, 0, 21, 255, 64, 0, 3, 255, 64, 0, 21, 255, 255, 255, 255,
                255, 64, 0, 21, 255, 64, 0, 3, 255, 64, 0, 21, 255, 64, 0, 3,
            ]
        );
    }

    #[test]
    fn sharpen_matches_the_pixel_golden_fixture() {
        let mut source = RgbaImage::new(3, 1);
        source.put_pixel(0, 0, Rgba([20, 80, 160, 255]));
        source.put_pixel(1, 0, Rgba([180, 120, 60, 192]));
        source.put_pixel(2, 0, Rgba([40, 200, 100, 128]));
        let mut horizontal = RgbaImage::new(3, 1);
        let mut output = RgbaImage::new(3, 1);
        sharpen(&source, &mut horizontal, &mut output, 0.8, 1.0);
        assert_eq!(
            output.as_raw(),
            &[10, 78, 166, 255, 206, 120, 46, 192, 23, 210, 105, 128]
        );
    }

    #[test]
    fn glow_spreads_visible_premultiplied_alpha_outside_the_source() {
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
        let horizontal = target.get_pixel(2, 3);
        let vertical = target.get_pixel(3, 2);
        assert!(horizontal[0] > 0 && horizontal[3] > 0);
        assert!(vertical[0] > 0 && vertical[3] > 0);
        assert_eq!(horizontal, vertical);
        assert!(target.get_pixel(2, 3)[3] > target.get_pixel(1, 3)[3]);
        assert_eq!(target.get_pixel(2, 3)[1], 0);
        assert_eq!(target.get_pixel(2, 3)[2], 0);
    }

    #[test]
    fn gaussian_kernels_are_reused_and_bounded() {
        let source = RgbaImage::from_pixel(3, 3, Rgba([255, 255, 255, 255]));
        let mut horizontal = RgbaImage::new(3, 3);
        let mut target = RgbaImage::new(3, 3);
        gaussian_blur(&source, &mut horizontal, &mut target, 2.0);
        let after_first = gaussian_kernel_cache_len();
        gaussian_blur(&source, &mut horizontal, &mut target, 2.0);
        assert_eq!(gaussian_kernel_cache_len(), after_first);
        for radius in 1..24 {
            gaussian_blur(&source, &mut horizontal, &mut target, f64::from(radius));
        }
        assert!(gaussian_kernel_cache_len() <= 16);
    }
}
