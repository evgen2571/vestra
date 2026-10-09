use std::{cell::RefCell, time::Instant};

use image::{GenericImage, Rgba, RgbaImage};
use vestra_core::plan::{ColourTransform, EffectOperation, EffectResource, EvaluatedEffect};

use crate::{
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
    profiling_enabled: bool,
    atlases: &[std::sync::Arc<crate::ascii::PreparedGlyphAtlas>],
) {
    for effect in effects {
        let plan = effect_pass_plan(effect);
        let started = profiling_enabled.then(Instant::now);
        execute_effect_pass_sequence(
            surfaces,
            plan.as_slice(),
            profiling_enabled.then_some(timings),
            matches!(
                effect,
                EvaluatedEffect::Glow { .. } | EvaluatedEffect::Bloom { .. }
            ),
            matches!(effect, EvaluatedEffect::Sharpen { .. }),
            atlases,
        );
        if let Some(started) = started {
            let elapsed = started.elapsed();
            timings.effect_execution += elapsed;
            match effect {
                EvaluatedEffect::GaussianBlur { .. } => timings.gaussian_blur += elapsed,
                EvaluatedEffect::ZoomBlur { .. } => timings.zoom_blur += elapsed,
                EvaluatedEffect::RadialBlur { .. } => timings.radial_blur += elapsed,
                EvaluatedEffect::Glow { .. } | EvaluatedEffect::Bloom { .. } => {
                    timings.bloom_glow += elapsed;
                }
                EvaluatedEffect::ChromaticAberration { .. } => {
                    timings.chromatic_aberration += elapsed;
                }
                EvaluatedEffect::Vignette { .. } => timings.vignette += elapsed,
                EvaluatedEffect::ColorAdjust { .. } => timings.color_adjust += elapsed,
                EvaluatedEffect::Sharpen { .. } => timings.sharpen += elapsed,
                EvaluatedEffect::CameraShake { .. } => {}
                _ => timings.other_effects += elapsed,
            }
        }
    }
}

fn execute_effect_pass_sequence(
    surfaces: &mut EffectSurfacePool,
    passes: &[EffectPass],
    mut timings: Option<&mut CpuHotPathTimings>,
    profile_bloom_passes: bool,
    profile_sharpen_passes: bool,
    atlases: &[std::sync::Arc<crate::ascii::PreparedGlyphAtlas>],
) {
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
        let started = timings.as_ref().map(|_| Instant::now());
        surfaces.run_pass(
            pass.inputs.primary(),
            pass.inputs.secondary(),
            pass.output,
            |source, secondary, target| {
                execute_effect_pass(source, secondary, target, pass, atlases)
            },
        );
        if let Some(started) = started
            && let Some(timings) = timings.as_deref_mut()
            && (profile_bloom_passes || profile_sharpen_passes)
        {
            let elapsed = started.elapsed();
            match (profile_bloom_passes, profile_sharpen_passes, pass.operation) {
                (true, _, EffectOperation::HighlightExtract { .. }) => {
                    timings.bloom_highlight_extract += elapsed;
                }
                (true, _, EffectOperation::GaussianHorizontal { .. })
                | (true, _, EffectOperation::GaussianVertical { .. }) => {
                    timings.bloom_gaussian_blur += elapsed;
                }
                (
                    true,
                    _,
                    EffectOperation::Composite {
                        mode: crate::render::effects::CompositeMode::Additive,
                        ..
                    },
                ) => timings.bloom_composite += elapsed,
                (false, true, EffectOperation::GaussianHorizontal { .. })
                | (false, true, EffectOperation::GaussianVertical { .. }) => {
                    timings.sharpen_gaussian += elapsed;
                }
                (
                    false,
                    true,
                    EffectOperation::Composite {
                        mode: crate::render::effects::CompositeMode::Unsharp,
                        ..
                    },
                ) => timings.sharpen_unsharp_composite += elapsed,
                _ => {}
            }
        }
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
    atlases: &[std::sync::Arc<crate::ascii::PreparedGlyphAtlas>],
) {
    match pass.operation {
        EffectOperation::AsciiAnalyze { parameters } => {
            super::ascii::analyze(source, target, parameters)
        }
        EffectOperation::AsciiResolve { parameters } => super::ascii::resolve(
            source,
            secondary.expect("ASCII analysis"),
            target,
            &atlases[parameters.atlas],
            parameters,
        ),
        EffectOperation::HalftoneAnalyze {
            cell_size,
            angle_degrees,
            mode,
        } => super::analog::analyze(source, target, cell_size, angle_degrees, mode),
        EffectOperation::Halftone { .. } => super::analog::halftone(
            source,
            secondary.expect("halftone analysis"),
            target,
            pass.operation,
        ),
        EffectOperation::PixelSort { .. } => {
            super::analog::pixel_sort(source, target, pass.operation)
        }
        EffectOperation::Crt { .. } => super::analog::crt(source, target, pass.operation),
        EffectOperation::PaletteMap {
            palette,
            amount,
            nearest,
        } => super::stylization::palette_map(source, target, &palette, amount, nearest),
        EffectOperation::OrderedDither {
            palette,
            amount,
            strength,
            matrix,
            scale,
        } => super::stylization::ordered_dither(
            source, target, &palette, amount, strength, matrix, scale,
        ),
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
    profiling_enabled: bool,
    atlases: &[std::sync::Arc<crate::ascii::PreparedGlyphAtlas>],
) {
    if effects
        .iter()
        .all(|effect| effect_pass_plan(effect).is_empty())
    {
        return;
    }
    let started = profiling_enabled.then(Instant::now);
    surfaces.begin_from(destination);
    if let Some(started) = started {
        timings.surface_copy += started.elapsed();
    }
    let started = profiling_enabled.then(Instant::now);
    apply_chain(surfaces, effects, timings, profiling_enabled, atlases);
    if let Some(started) = started {
        timings.global_post_effect += started.elapsed();
    }
    let started = profiling_enabled.then(Instant::now);
    surfaces.copy_to(destination);
    if let Some(started) = started {
        timings.surface_copy += started.elapsed();
    }
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

const NORMALIZED_CHANNELS: [f64; 256] = {
    let mut values = [0.0; 256];
    let mut index = 0;
    while index < values.len() {
        values[index] = index as f64 / 255.0;
        index += 1;
    }
    values
};

fn convolve(source: &RgbaImage, target: &mut RgbaImage, kernel: &GaussianKernel, horizontal: bool) {
    let width = source.width() as usize;
    let height = source.height() as usize;
    let row_stride = width * 4;
    let (left, right, top, bottom) = convolution_bounds(source, kernel.radius as usize, horizontal);
    let source_data = source.as_raw();
    let target_data: &mut [u8] = target.as_mut();
    if (left, right, top, bottom) != (0, width, 0, height) {
        target_data.fill(0);
    }
    for y in top..bottom {
        let source_row = y * row_stride;
        for x in left..right {
            let mut premultiplied = [0.0; 3];
            let mut alpha = 0.0;
            for (index, offset) in (-kernel.radius..=kernel.radius).enumerate() {
                let source_index = if horizontal {
                    let sample_x = (x as i32 + offset).clamp(0, width as i32 - 1) as usize;
                    source_row + sample_x * 4
                } else {
                    let sample_y = (y as i32 + offset).clamp(0, height as i32 - 1) as usize;
                    sample_y * row_stride + x * 4
                };
                let pixel = &source_data[source_index..source_index + 4];
                let weight = kernel.weights[index];
                let sample_alpha = NORMALIZED_CHANNELS[usize::from(pixel[3])];
                alpha += sample_alpha * weight;
                for channel in 0..3 {
                    premultiplied[channel] +=
                        NORMALIZED_CHANNELS[usize::from(pixel[channel])] * sample_alpha * weight;
                }
            }
            let rgb = if alpha <= 0.000_000_1 {
                [0; 3]
            } else {
                premultiplied.map(|value| (value / alpha * 255.0).round().clamp(0.0, 255.0) as u8)
            };
            let target_index = source_row + x * 4;
            target_data[target_index..target_index + 4].copy_from_slice(&[
                rgb[0],
                rgb[1],
                rgb[2],
                (alpha * 255.0).round() as u8,
            ]);
        }
    }
}

fn convolution_bounds(
    source: &RgbaImage,
    radius: usize,
    horizontal: bool,
) -> (usize, usize, usize, usize) {
    let width = source.width() as usize;
    let height = source.height() as usize;
    if width == 0 || height == 0 {
        return (0, 0, 0, 0);
    }
    // Dense images rarely benefit from scanning for transparent margins.
    if [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ]
    .into_iter()
    .any(|(x, y)| source.get_pixel(x as u32, y as u32)[3] != 0)
    {
        return (0, width, 0, height);
    }
    let (mut left, mut right, mut top, mut bottom) = (width, 0, height, 0);
    for (y, row) in source.as_raw().chunks_exact(width * 4).enumerate() {
        if let Some(first) = row.chunks_exact(4).position(|pixel| pixel[3] != 0) {
            let last = row
                .chunks_exact(4)
                .rposition(|pixel| pixel[3] != 0)
                .expect("row contains a visible pixel");
            left = left.min(first);
            right = right.max(last + 1);
            top = top.min(y);
            bottom = y + 1;
        }
    }
    if right == 0 {
        return (0, 0, 0, 0);
    }
    // Keep original canvas coordinates and edge clamping. A finite kernel can
    // only spread alpha by its support along the current pass's axis.
    if horizontal {
        left = left.saturating_sub(radius);
        right = right.saturating_add(radius).min(width);
    } else {
        top = top.saturating_sub(radius);
        bottom = bottom.saturating_add(radius).min(height);
    }
    (left, right, top, bottom)
}

#[cfg(test)]
fn convolve_reference(
    source: &RgbaImage,
    target: &mut RgbaImage,
    kernel: &GaussianKernel,
    horizontal: bool,
) {
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

    fn canonical_stylization(value: serde_json::Value) -> EvaluatedEffect {
        let mut project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/wgpu-small-rgba.json"
        ))
        .expect("canonical fixture");
        let id = value["id"].as_str().expect("test effect id").to_owned();
        project["visual"]["post_effects"] = serde_json::json!([value]);
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/stylization/cpu-tests");
        std::fs::create_dir_all(&directory).expect("test directory");
        let path = directory.join(format!("{id}.json"));
        std::fs::write(
            &path,
            serde_json::to_vec(&project).expect("serialize fixture"),
        )
        .expect("write fixture");
        let input = crate::test_support::load_and_validate(
            &path,
            &crate::test_support::ValidationOptions::default(),
        )
        .expect("stylization canonical model accepted");
        let plan =
            vestra_core::plan::compile(input, Default::default()).expect("stylization compiles");
        let signals = vestra_core::plan::PreparedScalarSignals::empty();
        vestra_core::plan::evaluate_effect(
            &plan.post_effects[0].effect,
            0,
            1_000_000_000,
            &vestra_core::plan::EvaluationContext::new(&signals),
        )
        .expect("stylization evaluates")
    }

    #[test]
    fn palette_gradient_maps_authored_tones_and_preserves_alpha_and_hidden_rgb() {
        let effect = canonical_stylization(serde_json::json!({
            "type":"palette_map", "id":"gradient",
            "palette":["#100020","#e0ff80"], "mode":"gradient",
            "amount":{"base_value":1.0}, "phase":{"base_value":0.0}
        }));
        let source = RgbaImage::from_raw(
            4,
            1,
            vec![
                0, 0, 0, 255, 128, 128, 128, 128, 255, 255, 255, 255, 128, 0, 255, 0,
            ],
        )
        .expect("source pixels");
        let output = render_effect(&source, effect);
        assert_eq!(
            output.as_raw(),
            &[
                16, 0, 32, 255, 120, 128, 80, 128, 224, 255, 128, 255, 128, 0, 255, 0
            ]
        );
    }

    #[test]
    fn ordered_dither_has_literal_bayer_ranks_and_palette_independent_structure() {
        for (id, palette, dark, light) in [
            (
                "mono",
                ["#000000", "#ffffff"],
                [0, 0, 0, 128],
                [255, 255, 255, 128],
            ),
            (
                "cool",
                ["#101020", "#30e0ff"],
                [16, 16, 32, 128],
                [48, 224, 255, 128],
            ),
            (
                "warm",
                ["#200800", "#ffbc60"],
                [32, 8, 0, 128],
                [255, 188, 96, 128],
            ),
        ] {
            let effect = canonical_stylization(serde_json::json!({
                "type":"ordered_dither", "id":id, "palette":palette,
                "mode":"nearest", "amount":{"base_value":1.0},
                "strength":{"base_value":1.0}, "phase":{"base_value":0.0},
                "matrix":"bayer2", "scale":1
            }));
            let source = RgbaImage::from_pixel(2, 2, Rgba([128, 128, 128, 128]));
            let output = render_effect(&source, effect);
            assert_eq!(
                output.pixels().map(|p| p.0).collect::<Vec<_>>(),
                vec![dark, light, light, dark],
                "{id}"
            );
        }
    }

    #[test]
    fn dither_scale_expands_threshold_cells_without_changing_tone_or_alpha() {
        let effect = canonical_stylization(serde_json::json!({
            "type":"ordered_dither", "id":"scale", "palette":["#000000","#ffffff"],
            "mode":"nearest", "amount":{"base_value":1.0},
            "strength":{"base_value":1.0}, "phase":{"base_value":0.0},
            "matrix":"bayer2", "scale":2
        }));
        let mut source = RgbaImage::from_pixel(4, 4, Rgba([128, 128, 128, 255]));
        source.put_pixel(3, 3, Rgba([99, 11, 240, 0]));
        let output = render_effect(&source, effect);
        let values = output.pixels().map(|p| p[0]).collect::<Vec<_>>();
        assert_eq!(
            values,
            vec![
                0, 0, 255, 255, 0, 0, 255, 255, 255, 255, 0, 0, 255, 255, 0, 99
            ]
        );
        assert_eq!(output.get_pixel(3, 3), source.get_pixel(3, 3));
    }

    fn render_effect(source: &RgbaImage, effect: EvaluatedEffect) -> RgbaImage {
        let mut surfaces = EffectSurfacePool::new(source.width(), source.height());
        surfaces.begin_from(source);
        let mut timings = CpuHotPathTimings::default();
        apply_chain(&mut surfaces, &[effect], &mut timings, true, &[]);
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
    fn optimized_gaussian_convolution_is_byte_identical_to_reference() {
        for (width, height) in [(1, 1), (1, 257), (257, 1), (17, 19)] {
            let mut source = RgbaImage::new(width, height);
            for (index, pixel) in source.pixels_mut().enumerate() {
                *pixel = Rgba([
                    (index * 17) as u8,
                    (index * 31) as u8,
                    (index * 47) as u8,
                    (index * 29) as u8,
                ]);
            }

            for radius in [1.0, 2.5, 4.0, 16.0, 32.0] {
                let kernel = GaussianKernel::new(radius);
                for horizontal in [true, false] {
                    let mut optimized = RgbaImage::new(source.width(), source.height());
                    let mut reference = RgbaImage::new(source.width(), source.height());
                    convolve(&source, &mut optimized, &kernel, horizontal);
                    convolve_reference(&source, &mut reference, &kernel, horizontal);
                    assert_eq!(
                        optimized, reference,
                        "size={width}x{height}, radius={radius}, horizontal={horizontal}"
                    );
                }
            }
        }
    }

    #[test]
    fn sparse_gaussian_matches_full_frame_reference_with_reused_targets() {
        let mut actual = RgbaImage::from_pixel(19, 13, Rgba([255; 4]));
        let mut expected = actual.clone();
        let mut actual_second = actual.clone();
        let mut expected_second = actual.clone();
        for (left, top, right, bottom) in [
            (3, 2, 15, 10),
            (8, 6, 9, 7),
            (0, 0, 1, 1),
            (18, 12, 19, 13),
            (0, 5, 19, 6),
            (5, 0, 6, 13),
            (1, 1, 18, 12),
            (0, 0, 0, 0),
        ] {
            let source = RgbaImage::from_fn(19, 13, |x, y| {
                let alpha = if (left..right).contains(&x)
                    && (top..bottom).contains(&y)
                    && (left != 1 || !(3..=16).contains(&x))
                {
                    ((x * 17 + y * 23) % 255 + 1) as u8
                } else {
                    0
                };
                Rgba([211, (x * 13) as u8, (y * 19) as u8, alpha])
            });
            for radius in [1.0, 2.5, 8.0, 32.0] {
                let kernel = GaussianKernel::new(radius);
                for horizontal in [true, false] {
                    convolve(&source, &mut actual, &kernel, horizontal);
                    convolve_reference(&source, &mut expected, &kernel, horizontal);
                    assert_eq!(
                        actual, expected,
                        "region {left},{top},{right},{bottom}, radius {radius}, horizontal {horizontal}"
                    );
                }
                convolve(&source, &mut actual, &kernel, true);
                convolve(&actual, &mut actual_second, &kernel, false);
                convolve_reference(&source, &mut expected, &kernel, true);
                convolve_reference(&expected, &mut expected_second, &kernel, false);
                assert_eq!(actual_second, expected_second, "two-pass radius {radius}");
            }
        }
    }

    #[test]
    #[ignore = "manual release Gaussian bounds benchmark"]
    fn gaussian_bounds_benchmark() {
        use std::{hint::black_box, time::Instant};
        let (width, height) = (1280, 720);
        for pattern in ["opaque", "near_full", "islands", "sparse"] {
            let source = RgbaImage::from_fn(width, height, |x, y| {
                let visible = match pattern {
                    "opaque" => true,
                    "near_full" => x > 0 && y > 0 && x + 1 < width && y + 1 < height,
                    "islands" => {
                        ((1..5).contains(&x) && (1..5).contains(&y))
                            || ((width - 5..width - 1).contains(&x)
                                && (height - 5..height - 1).contains(&y))
                    }
                    _ => {
                        (width / 3..width * 2 / 3).contains(&x)
                            && (height / 3..height * 2 / 3).contains(&y)
                    }
                };
                Rgba([211, x as u8, y as u8, if visible { 255 } else { 0 }])
            });
            let mut intermediate = RgbaImage::new(width, height);
            let mut target = RgbaImage::new(width, height);
            for radius in [1.0, 8.0] {
                let kernel = GaussianKernel::new(radius);
                convolve(&source, &mut intermediate, &kernel, true);
                convolve(&intermediate, &mut target, &kernel, false);
                for sample in 0..5 {
                    let started = Instant::now();
                    convolve(&source, &mut intermediate, &kernel, true);
                    convolve(&intermediate, &mut target, &kernel, false);
                    black_box(target.as_raw());
                    println!(
                        "gaussian_bounds pattern={pattern} width={width} height={height} radius={radius} sample={sample} wall_ms={:.3} scope=prepared_two_pass",
                        started.elapsed().as_secs_f64() * 1000.0
                    );
                }
            }
        }
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

        execute_effect_pass_sequence(&mut surfaces, &passes, None, false, false, &[]);

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
            true,
            &[],
        );

        assert_eq!(surfaces.stats().copy_bytes, copied_before_effect);
        assert_eq!(surfaces.stats().reuses, 4);
        assert_eq!(surfaces.stats().allocations, 3);
        assert_eq!(surfaces.stats().retained_buffers, 3);
    }
}
