use std::{cell::RefCell, time::Instant};

use image::{GenericImage, Rgba, RgbaImage};

use crate::{
    plan::{ColourTransform, EffectOperation, EffectResource, EvaluatedEffect},
    render::effects::{
        EffectPass, canonical_gaussian_radius, effect_pass_plan, gaussian_radius_is_identity,
        sampling_blur_radius_is_identity,
    },
    render::metrics::CpuHotPathTimings,
};

use super::surfaces::EffectSurfacePool;

/// Executes the backend-neutral logical pass plan against CPU surfaces.
pub(super) fn apply_chain(
    surfaces: &mut EffectSurfacePool,
    effects: &[EvaluatedEffect],
    timings: &mut CpuHotPathTimings,
) {
    for effect in effects {
        let plan = effect_pass_plan(effect);
        let started = Instant::now();
        execute_effect_pass_sequence(surfaces, plan.as_slice());
        let elapsed = started.elapsed();
        timings.effect_execution += elapsed;
        match effect {
            EvaluatedEffect::GaussianBlur { .. } => timings.gaussian_blur += elapsed,
            EvaluatedEffect::ZoomBlur { .. } => timings.zoom_blur += elapsed,
            EvaluatedEffect::Glow { .. } | EvaluatedEffect::Bloom { .. } => {
                timings.bloom_glow += elapsed;
            }
            EvaluatedEffect::CameraShake { .. } => {}
            _ => timings.other_effects += elapsed,
        }
    }
}

fn execute_effect_pass_sequence(surfaces: &mut EffectSurfacePool, passes: &[EffectPass]) {
    if passes.is_empty() {
        return;
    }
    let retains_original = passes.iter().any(|pass| pass.inputs.uses_original());
    surfaces.begin_effect(retains_original);
    for (index, pass) in passes.iter().enumerate() {
        let remaining = &passes[index..];
        for temporary in [EffectResource::Temporary0, EffectResource::Temporary1] {
            if !resource_value_is_live(temporary, remaining) {
                surfaces.release_temporary(temporary);
            }
        }
        surfaces.run_pass(
            pass.inputs.primary(),
            pass.inputs.secondary(),
            pass.output,
            |source, secondary, target| execute_effect_pass(source, secondary, target, pass),
        );
    }
}

/// Returns whether the resource value present before `remaining[0]` will be
/// read before that logical resource is overwritten. This is sufficient for
/// the ordered effect-pass IR and lets the CPU backend recycle dead temporaries
/// without introducing a graph allocator.
fn resource_value_is_live(resource: EffectResource, remaining: &[EffectPass]) -> bool {
    for pass in remaining {
        if pass.inputs.primary() == resource || pass.inputs.secondary() == Some(resource) {
            return true;
        }
        if pass.output == resource {
            return false;
        }
    }
    false
}

fn execute_effect_pass(
    source: &RgbaImage,
    secondary: Option<&RgbaImage>,
    target: &mut RgbaImage,
    pass: &EffectPass,
) {
    match pass.operation {
        EffectOperation::ApplyColourTransform { transform } => {
            apply_colour_transform(source, target, transform)
        }
        EffectOperation::GaussianHorizontal { radius } => {
            gaussian_pass(source, target, radius, true)
        }
        EffectOperation::GaussianVertical { radius } => {
            gaussian_pass(source, target, radius, false)
        }
        EffectOperation::HighlightExtract { threshold, colour } => {
            highlight_extract(source, target, threshold, colour)
        }
        EffectOperation::Composite { mode, amount } => {
            let base = source;
            let overlay = secondary.expect("composite passes declare two inputs");
            match mode {
                crate::render::effects::CompositeMode::Additive => {
                    glow_composite(base, overlay, target, amount)
                }
                crate::render::effects::CompositeMode::Unsharp => {
                    unsharp_composite(base, overlay, target, amount)
                }
            }
        }
        EffectOperation::DirectionalBlur {
            radius,
            angle_degrees,
        } => blur(source, target, radius, Some(angle_degrees), None),
        EffectOperation::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => super::zoom_blur::apply(source, target, radius, samples, anchor, direction),
        EffectOperation::ChromaticAberration {
            amount,
            angle_degrees,
        } => super::chromatic::apply(source, target, amount, angle_degrees),
        EffectOperation::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => super::vignette::apply(source, target, amount, radius, softness, colour),
        EffectOperation::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => super::colour_adjust::apply(source, target, exposure, gamma, black_point, white_point),
        EffectOperation::MotionBlur {
            radius,
            angle_degrees,
            samples,
        } => blur(source, target, radius, Some(angle_degrees), Some(samples)),
    }
}

fn gaussian_pass(source: &RgbaImage, target: &mut RgbaImage, radius: f64, horizontal: bool) {
    if gaussian_radius_is_identity(radius) {
        target.copy_from(source, 0, 0).expect("matching surfaces");
        return;
    }
    with_gaussian_kernel(radius, |kernel| {
        convolve(source, target, kernel, horizontal)
    });
}

fn highlight_extract(source: &RgbaImage, target: &mut RgbaImage, threshold: f64, colour: [u8; 4]) {
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
}

fn glow_composite(original: &RgbaImage, bloom: &RgbaImage, target: &mut RgbaImage, intensity: f64) {
    for ((base, glow), output) in original
        .pixels()
        .zip(bloom.pixels())
        .zip(target.pixels_mut())
    {
        let base_alpha = f64::from(base[3]) / 255.0;
        let glow_alpha = (f64::from(glow[3]) / 255.0 * intensity).clamp(0.0, 1.0);
        let alpha = base_alpha + glow_alpha * (1.0 - base_alpha);
        let rgb = if alpha <= 0.000_000_1 {
            [0; 3]
        } else {
            std::array::from_fn(|channel| {
                ((f64::from(base[channel]) / 255.0 * base_alpha
                    + f64::from(glow[channel]) / 255.0 * glow_alpha)
                    / alpha
                    * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
        };
        *output = Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]);
    }
}

fn unsharp_composite(
    original: &RgbaImage,
    blurred: &RgbaImage,
    target: &mut RgbaImage,
    amount: f64,
) {
    for ((base, blurred), output) in original
        .pixels()
        .zip(blurred.pixels())
        .zip(target.pixels_mut())
    {
        let mut result = *blurred;
        for channel in 0..3 {
            result[channel] = (f64::from(base[channel])
                + (f64::from(base[channel]) - f64::from(blurred[channel])) * amount)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        result[3] = base[3];
        *output = result;
    }
}

pub(super) fn apply_to(
    surfaces: &mut EffectSurfacePool,
    destination: &mut RgbaImage,
    effects: &[EvaluatedEffect],
    timings: &mut CpuHotPathTimings,
) {
    if effects
        .iter()
        .all(|effect| effect_pass_plan(effect).is_empty())
    {
        return;
    }
    let started = Instant::now();
    surfaces.begin_from(destination);
    timings.surface_copy += started.elapsed();
    let started = Instant::now();
    apply_chain(surfaces, effects, timings);
    timings.global_post_effect += started.elapsed();
    let started = Instant::now();
    surfaces.copy_to(destination);
    timings.surface_copy += started.elapsed();
}

fn apply_colour_transform(source: &RgbaImage, target: &mut RgbaImage, transform: ColourTransform) {
    map_pixels(source, target, |mut pixel| {
        let rgb = [
            f64::from(pixel[0]),
            f64::from(pixel[1]),
            f64::from(pixel[2]),
        ];
        for channel in 0..3 {
            pixel[channel] = (transform.matrix[channel][0] * rgb[0]
                + transform.matrix[channel][1] * rgb[1]
                + transform.matrix[channel][2] * rgb[2]
                + transform.offset[channel])
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        pixel
    });
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
    if sampling_blur_radius_is_identity(radius) {
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
    let key = (canonical_gaussian_radius(radius) * 4.0) as u16;
    GAUSSIAN_KERNEL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache
            .iter()
            .position(|(cached, _)| *cached == key)
            .unwrap_or_else(|| {
                if cache.len() == 16 {
                    cache.remove(0);
                }
                cache.push((key, GaussianKernel::new(canonical_gaussian_radius(radius))));
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

    fn render_effect(source: &RgbaImage, effect: EvaluatedEffect) -> RgbaImage {
        let mut surfaces = EffectSurfacePool::new(source.width(), source.height());
        surfaces.begin_from(source);
        let mut timings = CpuHotPathTimings::default();
        apply_chain(&mut surfaces, &[effect], &mut timings);
        let mut output = RgbaImage::new(source.width(), source.height());
        surfaces.copy_to(&mut output);
        output
    }

    #[test]
    fn gaussian_spreads_symmetrically_without_transparent_colour_halos() {
        let mut source = RgbaImage::from_pixel(7, 7, Rgba([255, 0, 0, 0]));
        source.put_pixel(3, 3, Rgba([255, 255, 255, 255]));
        let target = render_effect(&source, EvaluatedEffect::GaussianBlur { radius: 2.0 });

        assert_eq!(target.get_pixel(2, 3), target.get_pixel(4, 3));
        assert_eq!(target.get_pixel(3, 2), target.get_pixel(3, 4));
        assert!(target.get_pixel(2, 3)[3] > 0);
        assert_eq!(target.get_pixel(0, 0)[0], 0);
    }

    #[test]
    fn transparent_glow_matches_the_pixel_golden_fixture_through_pass_execution() {
        let mut source = RgbaImage::new(3, 3);
        source.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let output = render_effect(
            &source,
            EvaluatedEffect::Glow {
                threshold: 0.0,
                radius: 1.0,
                intensity: 1.0,
                colour: [255, 64, 0, 255],
            },
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
    fn sharpen_matches_the_pixel_golden_fixture_through_pass_execution() {
        let mut source = RgbaImage::new(3, 1);
        source.put_pixel(0, 0, Rgba([20, 80, 160, 255]));
        source.put_pixel(1, 0, Rgba([180, 120, 60, 192]));
        source.put_pixel(2, 0, Rgba([40, 200, 100, 128]));
        let output = render_effect(
            &source,
            EvaluatedEffect::Sharpen {
                amount: 0.8,
                radius: 1.0,
            },
        );

        assert_eq!(
            output.as_raw(),
            &[10, 78, 166, 255, 206, 120, 46, 192, 23, 210, 105, 128]
        );
    }

    #[test]
    fn glow_spreads_visible_premultiplied_alpha_outside_the_source() {
        let mut source = RgbaImage::new(7, 7);
        source.put_pixel(3, 3, Rgba([255, 255, 255, 255]));
        let target = render_effect(
            &source,
            EvaluatedEffect::Glow {
                threshold: 0.5,
                radius: 2.0,
                intensity: 1.0,
                colour: [255, 0, 0, 255],
            },
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
    fn gaussian_kernels_are_reused_and_bounded_through_pass_execution() {
        let source = RgbaImage::from_pixel(3, 3, Rgba([255, 255, 255, 255]));
        let _ = render_effect(&source, EvaluatedEffect::GaussianBlur { radius: 2.0 });
        let after_first = gaussian_kernel_cache_len();
        let _ = render_effect(&source, EvaluatedEffect::GaussianBlur { radius: 2.0 });
        assert_eq!(gaussian_kernel_cache_len(), after_first);
        for radius in 1..24 {
            let _ = render_effect(
                &source,
                EvaluatedEffect::GaussianBlur {
                    radius: f64::from(radius),
                },
            );
        }
        assert!(gaussian_kernel_cache_len() <= 16);
    }

    #[test]
    fn small_sampling_radius_remains_a_cpu_effect_for_directional_and_motion_blur() {
        let mut source = RgbaImage::new(3, 1);
        source.put_pixel(1, 0, Rgba([255, 255, 255, 255]));
        let mut directional = RgbaImage::new(3, 1);
        let mut motion = RgbaImage::new(3, 1);
        blur(&source, &mut directional, 0.12, Some(0.0), None);
        blur(&source, &mut motion, 0.12, Some(0.0), Some(9));
        assert_ne!(directional, source);
        assert_ne!(motion, source);
    }

    #[test]
    fn arbitrary_five_pass_sequence_executes_each_pass_in_order() {
        let passes = [
            EffectPass::new(
                EffectOperation::ApplyColourTransform {
                    transform: ColourTransform {
                        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                        offset: [1.0, 0.0, 0.0],
                    },
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ApplyColourTransform {
                    transform: ColourTransform {
                        matrix: [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                        offset: [0.0; 3],
                    },
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ApplyColourTransform {
                    transform: ColourTransform {
                        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.0]],
                        offset: [0.0; 3],
                    },
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ApplyColourTransform {
                    transform: ColourTransform {
                        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                        offset: [5.0, 6.0, 7.0],
                    },
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
            EffectPass::new(
                EffectOperation::ApplyColourTransform {
                    transform: ColourTransform {
                        matrix: [[1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                        offset: [0.0; 3],
                    },
                },
                EffectResource::Current,
                EffectResource::Current,
            ),
        ];
        let source = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
        let mut surfaces = EffectSurfacePool::new(1, 1);
        surfaces.begin_from(&source);

        execute_effect_pass_sequence(&mut surfaces, &passes);

        assert_eq!(surfaces.current().get_pixel(0, 0), &Rgba([42, 17, 67, 255]));
        assert_eq!(surfaces.stats().reuses, 5);
    }

    #[test]
    fn original_dependent_passes_pin_retained_storage_without_extra_frame_copy() {
        let mut source = RgbaImage::new(3, 3);
        source.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let mut surfaces = EffectSurfacePool::new(3, 3);
        surfaces.begin_from(&source);
        let copied_before_effect = surfaces.stats().copy_bytes;

        apply_chain(
            &mut surfaces,
            &[EvaluatedEffect::Glow {
                threshold: 0.0,
                radius: 1.0,
                intensity: 1.0,
                colour: [255, 64, 0, 255],
            }],
            &mut CpuHotPathTimings::default(),
        );

        assert_eq!(surfaces.stats().copy_bytes, copied_before_effect);
        assert_eq!(surfaces.stats().reuses, 4);
        assert_eq!(surfaces.stats().allocations, 3);
        assert_eq!(surfaces.stats().retained_buffers, 3);
    }
}
