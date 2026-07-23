use image::{GenericImage, Rgba, RgbaImage};

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{ColourTransform, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource},
    render::prepared::PreparedAssets,
};

/// Composites an immutable, backend-neutral frame program into a reusable buffer.
pub fn compose(
    frame: &EvaluatedFrame,
    assets: &mut PreparedAssets,
    canvas: &mut RgbaImage,
    surfaces: &mut EffectSurfacePool,
) {
    let _time = frame.time;
    if canvas.width() != frame.width || canvas.height() != frame.height {
        *canvas = RgbaImage::from_pixel(frame.width, frame.height, Rgba(frame.background));
    } else {
        for pixel in canvas.pixels_mut() {
            *pixel = Rgba(frame.background);
        }
    }
    surfaces.resize(frame.width, frame.height);
    for layer in &frame.layers {
        surfaces.clear();
        draw_layer(surfaces.current(), assets, layer);
        surfaces.apply(&layer.effects);
        blend_surface(canvas, surfaces.current(), layer.blend_mode, layer.opacity);
    }
    surfaces.apply_to(canvas, &frame.post_effects);
}

pub struct EffectSurfacePool {
    first: RgbaImage,
    second: RgbaImage,
    horizontal: RgbaImage,
    first_is_current: bool,
}
impl EffectSurfacePool {
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            first: RgbaImage::new(width, height),
            second: RgbaImage::new(width, height),
            horizontal: RgbaImage::new(width, height),
            first_is_current: true,
        }
    }
    fn resize(&mut self, width: u32, height: u32) {
        if self.first.width() != width || self.first.height() != height {
            self.first = RgbaImage::new(width, height);
            self.second = RgbaImage::new(width, height);
            self.horizontal = RgbaImage::new(width, height);
            self.first_is_current = true;
        }
    }
    fn current(&mut self) -> &mut RgbaImage {
        if self.first_is_current {
            &mut self.first
        } else {
            &mut self.second
        }
    }
    fn clear(&mut self) {
        for pixel in self.current().pixels_mut() {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }
    fn apply(&mut self, effects: &[EvaluatedEffect]) {
        for effect in effects {
            if !matches!(effect, EvaluatedEffect::CameraShake { .. }) {
                self.run(effect);
            }
        }
    }
    fn apply_to(&mut self, destination: &mut RgbaImage, effects: &[EvaluatedEffect]) {
        self.first
            .copy_from(destination, 0, 0)
            .expect("matching effect surface dimensions");
        self.first_is_current = true;
        self.apply(effects);
        destination
            .copy_from(self.current(), 0, 0)
            .expect("matching effect surface dimensions");
    }
    fn run(&mut self, effect: &EvaluatedEffect) {
        let (source, target) = if self.first_is_current {
            (&self.first, &mut self.second)
        } else {
            (&self.second, &mut self.first)
        };
        match effect {
            EvaluatedEffect::GaussianBlur { radius } => {
                crate::render::effects::gaussian_blur(source, &mut self.horizontal, target, *radius)
            }
            EvaluatedEffect::Glow {
                threshold,
                radius,
                intensity,
                colour,
            } => crate::render::effects::glow(
                source,
                &mut self.horizontal,
                target,
                *threshold,
                *radius,
                *intensity,
                *colour,
            ),
            EvaluatedEffect::Sharpen { amount, radius } => crate::render::effects::sharpen(
                source,
                &mut self.horizontal,
                target,
                *amount,
                *radius,
            ),
            _ => apply_effect(source, target, effect),
        }
        self.first_is_current = !self.first_is_current;
    }
}

fn draw_layer(canvas: &mut RgbaImage, assets: &mut PreparedAssets, layer: &EvaluatedLayer) {
    match &layer.source {
        EvaluatedSource::SolidColor { colour } => {
            fill_solid(canvas, *colour, 1.0, ColourTransform::default())
        }
        EvaluatedSource::Image {
            asset_index,
            crop,
            sizing,
            cacheable_crop,
            transform,
        } => {
            let (source, crop) =
                if *cacheable_crop && let Some(source) = assets.crop(*asset_index, *crop) {
                    (
                        source,
                        Crop {
                            x: 0.0,
                            y: 0.0,
                            width: 1.0,
                            height: 1.0,
                        },
                    )
                } else {
                    (assets.image(*asset_index), *crop)
                };
            let (source_width, source_height) = sizing_dimensions(
                sizing,
                crop.width * f64::from(source.width()),
                crop.height * f64::from(source.height()),
                canvas.width(),
                canvas.height(),
            );
            if transform.is_valid() {
                draw_image(
                    canvas,
                    source,
                    crop,
                    source_width,
                    source_height,
                    *transform,
                    1.0,
                    ColourTransform::default(),
                );
            }
        }
    }
}

fn fill_solid(
    canvas: &mut RgbaImage,
    colour: [u8; 4],
    opacity: f64,
    colour_transform: ColourTransform,
) {
    let source = apply_colour_transform(Rgba(colour), colour_transform);
    for destination in canvas.pixels_mut() {
        *destination = source_over(*destination, source, opacity);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_image(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    crop: Crop,
    effective_width: f64,
    effective_height: f64,
    transform: Transform2D,
    opacity: f64,
    colour_transform: ColourTransform,
) {
    let (min_x, max_x, min_y, max_y) = visible_bounds(
        transform,
        effective_width,
        effective_height,
        canvas.width(),
        canvas.height(),
    );
    let inverse = InverseAffine::for_transform(
        transform,
        canvas.width(),
        canvas.height(),
        effective_width,
        effective_height,
    );
    for y in min_y..max_y {
        let mut mapped = inverse.map(f64::from(min_x) + 0.5, f64::from(y) + 0.5);
        for x in min_x..max_x {
            if mapped.x >= 0.0
                && mapped.y >= 0.0
                && mapped.x < effective_width
                && mapped.y < effective_height
            {
                let source_x = crop.x * f64::from(source.width())
                    + mapped.x / effective_width * crop.width * f64::from(source.width());
                let source_y = crop.y * f64::from(source.height())
                    + mapped.y / effective_height * crop.height * f64::from(source.height());
                let sampled = apply_colour_transform(
                    sample_bilinear(source, source_x, source_y),
                    colour_transform,
                );
                let destination = canvas.get_pixel_mut(x, y);
                *destination = source_over(*destination, sampled, opacity);
            }
            mapped.x += inverse.m00;
            mapped.y += inverse.m10;
        }
    }
}

/// Inverse transform from canvas coordinates to an unscaled image coordinate.
/// It is built once per layer and lets a scanline advance with two additions.
#[derive(Clone, Copy, Debug)]
struct InverseAffine {
    m00: f64,
    m01: f64,
    m02: f64,
    m10: f64,
    m11: f64,
    m12: f64,
}

impl InverseAffine {
    fn for_transform(
        transform: Transform2D,
        canvas_width: u32,
        canvas_height: u32,
        source_width: f64,
        source_height: f64,
    ) -> Self {
        let (sine, cosine) = transform.rotation_radians.sin_cos();
        let destination_x = transform.position.x * f64::from(canvas_width);
        let destination_y = transform.position.y * f64::from(canvas_height);
        let anchor_x = transform.anchor.x * source_width;
        let anchor_y = transform.anchor.y * source_height;
        let m00 = cosine / transform.scale.x;
        let m01 = sine / transform.scale.x;
        let m10 = -sine / transform.scale.y;
        let m11 = cosine / transform.scale.y;
        Self {
            m00,
            m01,
            m02: anchor_x - m00 * destination_x - m01 * destination_y,
            m10,
            m11,
            m12: anchor_y - m10 * destination_x - m11 * destination_y,
        }
    }

    fn map(self, x: f64, y: f64) -> crate::domain::Point {
        crate::domain::Point {
            x: self.m00 * x + self.m01 * y + self.m02,
            y: self.m10 * x + self.m11 * y + self.m12,
        }
    }
}

fn visible_bounds(
    transform: Transform2D,
    source_width: f64,
    source_height: f64,
    canvas_width: u32,
    canvas_height: u32,
) -> (u32, u32, u32, u32) {
    let (sine, cosine) = transform.rotation_radians.sin_cos();
    let anchor_x = transform.anchor.x * source_width;
    let anchor_y = transform.anchor.y * source_height;
    let destination_x = transform.position.x * f64::from(canvas_width);
    let destination_y = transform.position.y * f64::from(canvas_height);
    let mut minimum_x = f64::INFINITY;
    let mut maximum_x = f64::NEG_INFINITY;
    let mut minimum_y = f64::INFINITY;
    let mut maximum_y = f64::NEG_INFINITY;
    for (x, y) in [
        (0.0, 0.0),
        (source_width, 0.0),
        (0.0, source_height),
        (source_width, source_height),
    ] {
        let local_x = (x - anchor_x) * transform.scale.x;
        let local_y = (y - anchor_y) * transform.scale.y;
        let x = destination_x + cosine * local_x - sine * local_y;
        let y = destination_y + sine * local_x + cosine * local_y;
        minimum_x = minimum_x.min(x);
        maximum_x = maximum_x.max(x);
        minimum_y = minimum_y.min(y);
        maximum_y = maximum_y.max(y);
    }
    (
        minimum_x.floor().max(0.0) as u32,
        maximum_x.ceil().clamp(0.0, f64::from(canvas_width)) as u32,
        minimum_y.floor().max(0.0) as u32,
        maximum_y.ceil().clamp(0.0, f64::from(canvas_height)) as u32,
    )
}

fn sizing_dimensions(
    sizing: &crate::plan::CompiledSizing,
    source_width: f64,
    source_height: f64,
    canvas_width: u32,
    canvas_height: u32,
) -> (f64, f64) {
    match sizing {
        crate::plan::CompiledSizing::Original => (source_width, source_height),
        crate::plan::CompiledSizing::Stretch { width, height } => {
            (f64::from(*width), f64::from(*height))
        }
        crate::plan::CompiledSizing::Scale(scale) => (source_width * scale, source_height * scale),
        crate::plan::CompiledSizing::Fit | crate::plan::CompiledSizing::Cover => {
            let horizontal = f64::from(canvas_width) / source_width;
            let vertical = f64::from(canvas_height) / source_height;
            let factor = if matches!(sizing, crate::plan::CompiledSizing::Fit) {
                horizontal.min(vertical)
            } else {
                horizontal.max(vertical)
            };
            (source_width * factor, source_height * factor)
        }
    }
}

fn sample_bilinear(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let x = x - 0.5;
    let y = y - 0.5;
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let mut result = [0.0; 4];
    for (offset_x, weight_x) in [(0_i64, 1.0 - tx), (1, tx)] {
        for (offset_y, weight_y) in [(0_i64, 1.0 - ty), (1, ty)] {
            let sample_x = x0 + offset_x;
            let sample_y = y0 + offset_y;
            if sample_x < 0
                || sample_y < 0
                || sample_x >= i64::from(image.width())
                || sample_y >= i64::from(image.height())
            {
                continue;
            }
            let sample = image.get_pixel(sample_x as u32, sample_y as u32);
            let weight = weight_x * weight_y;
            for channel in 0..4 {
                result[channel] += f64::from(sample[channel]) * weight;
            }
        }
    }
    Rgba(result.map(|channel| channel.round().clamp(0.0, 255.0) as u8))
}

fn apply_colour_transform(mut pixel: Rgba<u8>, transform: ColourTransform) -> Rgba<u8> {
    let input = [
        f64::from(pixel[0]),
        f64::from(pixel[1]),
        f64::from(pixel[2]),
    ];
    for channel in 0..3 {
        pixel[channel] = (transform.matrix[channel][0] * input[0]
            + transform.matrix[channel][1] * input[1]
            + transform.matrix[channel][2] * input[2]
            + transform.offset[channel])
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    pixel
}

pub fn source_over(destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) -> Rgba<u8> {
    let source_alpha = f64::from(source[3]) / 255.0 * opacity;
    let destination_alpha = f64::from(destination[3]) / 255.0;
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        result[channel] = ((f64::from(source[channel]) * source_alpha
            + f64::from(destination[channel]) * destination_alpha * (1.0 - source_alpha))
            / alpha)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba(result)
}

fn blend_surface(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    mode: crate::project::BlendMode,
    opacity: f64,
) {
    for (destination, source) in canvas.pixels_mut().zip(source.pixels()) {
        *destination = blend_pixel(*destination, *source, mode, opacity);
    }
}

fn blend_pixel(
    destination: Rgba<u8>,
    source: Rgba<u8>,
    mode: crate::project::BlendMode,
    opacity: f64,
) -> Rgba<u8> {
    if matches!(mode, crate::project::BlendMode::Normal) {
        return source_over(destination, source, opacity);
    }
    let sa = f64::from(source[3]) / 255.0 * opacity;
    let da = f64::from(destination[3]) / 255.0;
    let alpha = sa + da * (1.0 - sa);
    if alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        let s = f64::from(source[channel]) / 255.0;
        let d = f64::from(destination[channel]) / 255.0;
        let blend = match mode {
            crate::project::BlendMode::Normal => s,
            crate::project::BlendMode::Add => (s + d).min(1.0),
            crate::project::BlendMode::Screen => 1.0 - (1.0 - s) * (1.0 - d),
            crate::project::BlendMode::Multiply => s * d,
            crate::project::BlendMode::Overlay => {
                if d <= 0.5 {
                    2.0 * s * d
                } else {
                    1.0 - 2.0 * (1.0 - s) * (1.0 - d)
                }
            }
        };
        let premultiplied = blend * sa * da + s * sa * (1.0 - da) + d * da * (1.0 - sa);
        result[channel] = (premultiplied / alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round() as u8;
    Rgba(result)
}

fn apply_effect(source: &RgbaImage, target: &mut RgbaImage, effect: &EvaluatedEffect) {
    match effect {
        EvaluatedEffect::Brightness { amount } => map_pixels(source, target, |mut p, _x, _y| {
            for channel in 0..3 {
                p[channel] = (f64::from(p[channel]) + amount * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            p
        }),
        EvaluatedEffect::Contrast { amount } => map_pixels(source, target, |mut p, _x, _y| {
            for channel in 0..3 {
                p[channel] = ((f64::from(p[channel]) - 128.0) * amount + 128.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            p
        }),
        EvaluatedEffect::Saturation { amount } => map_pixels(source, target, |mut p, _x, _y| {
            let l = f64::from(p[0]) * 0.2126 + f64::from(p[1]) * 0.7152 + f64::from(p[2]) * 0.0722;
            for c in 0..3 {
                p[c] = (l * (1.0 - amount) + f64::from(p[c]) * amount)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            p
        }),
        EvaluatedEffect::Tint { colour, amount } => map_pixels(source, target, |mut p, _x, _y| {
            let t = amount.clamp(0.0, 1.0);
            for c in 0..3 {
                p[c] = (f64::from(p[c]) * (1.0 - t) + f64::from(colour[c]) * t).round() as u8;
            }
            p
        }),
        EvaluatedEffect::GaussianBlur { .. }
        | EvaluatedEffect::Glow { .. }
        | EvaluatedEffect::Sharpen { .. } => {
            unreachable!("multi-pass effects are dispatched by the surface pool")
        }
        EvaluatedEffect::DirectionalBlur {
            radius,
            angle_degrees,
        }
        | EvaluatedEffect::MotionBlur {
            radius,
            angle_degrees,
            ..
        } => blur(source, target, *radius, Some(*angle_degrees)),
        EvaluatedEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => chromatic(source, target, *amount, *angle_degrees),
        EvaluatedEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => vignette(source, target, *amount, *radius, *softness, *colour),
        EvaluatedEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => colour_adjust(
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
    mut map: impl FnMut(Rgba<u8>, u32, u32) -> Rgba<u8>,
) {
    for (x, y, pixel) in source.enumerate_pixels() {
        target.put_pixel(x, y, map(*pixel, x, y));
    }
}
fn sample_edge(image: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    sample_bilinear(
        image,
        x.clamp(0.5, f64::from(image.width()) - 0.5),
        y.clamp(0.5, f64::from(image.height()) - 0.5),
    )
}
fn blur(source: &RgbaImage, target: &mut RgbaImage, radius: f64, direction: Option<f64>) {
    if radius <= 0.01 {
        target.copy_from(source, 0, 0).expect("same dimensions");
        return;
    }
    let radius = radius.clamp(0.0, 32.0);
    let samples = (radius.ceil() as i32 * 2 + 1).clamp(3, 33);
    let (dx, dy) = direction.map_or((1.0, 0.0), |degrees| {
        let radians = degrees.to_radians();
        (radians.cos(), radians.sin())
    });
    for (x, y, _) in source.enumerate_pixels() {
        let mut sum = [0.0; 4];
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
            let pixel = sample_edge(source, sx + 0.5, sy + 0.5);
            for c in 0..4 {
                sum[c] += f64::from(pixel[c]);
            }
        }
        target.put_pixel(
            x,
            y,
            Rgba(sum.map(|v| (v / f64::from(samples)).round() as u8)),
        );
    }
}
fn chromatic(source: &RgbaImage, target: &mut RgbaImage, amount: f64, angle: f64) {
    if amount <= 0.0 {
        target.copy_from(source, 0, 0).expect("same dimensions");
        return;
    }
    let r = angle.to_radians();
    let (dx, dy) = (r.cos() * amount, r.sin() * amount);
    for (x, y, p) in source.enumerate_pixels() {
        let left = sample_edge(source, f64::from(x) - dx + 0.5, f64::from(y) - dy + 0.5);
        let right = sample_edge(source, f64::from(x) + dx + 0.5, f64::from(y) + dy + 0.5);
        target.put_pixel(x, y, Rgba([left[0], p[1], right[2], p[3]]));
    }
}
fn vignette(
    source: &RgbaImage,
    target: &mut RgbaImage,
    amount: f64,
    radius: f64,
    softness: f64,
    colour: [u8; 4],
) {
    let w = f64::from(source.width());
    let h = f64::from(source.height());
    for (x, y, p) in source.enumerate_pixels() {
        let dx = (f64::from(x) + 0.5 - w / 2.0) / (w.min(h) / 2.0);
        let dy = (f64::from(y) + 0.5 - h / 2.0) / (w.min(h) / 2.0);
        let distance = (dx * dx + dy * dy).sqrt();
        let edge = ((distance - radius) / (softness.max(0.001))).clamp(0.0, 1.0);
        let t = (amount * edge).clamp(0.0, 1.0);
        let mut out = *p;
        for c in 0..3 {
            out[c] = (f64::from(p[c]) * (1.0 - t) + f64::from(colour[c]) * t).round() as u8;
        }
        target.put_pixel(x, y, out);
    }
}
fn colour_adjust(
    source: &RgbaImage,
    target: &mut RgbaImage,
    exposure: f64,
    gamma: f64,
    black: f64,
    white: f64,
) {
    let scale = 1.0 / (white - black).max(0.000_1);
    map_pixels(source, target, |mut p, _, _| {
        for c in 0..3 {
            let v = (((f64::from(p[c]) / 255.0) * 2f64.powf(exposure) - black) * scale)
                .clamp(0.0, 1.0)
                .powf(1.0 / gamma.max(0.001));
            p[c] = (v * 255.0).round() as u8;
        }
        p
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::EvaluatedEffect;

    fn apply_sequential(mut rgb: [f64; 3], effects: &[EvaluatedEffect]) -> [f64; 3] {
        for effect in effects {
            match effect {
                EvaluatedEffect::Brightness { amount } => {
                    rgb = rgb.map(|channel| channel + amount * 255.0);
                }
                EvaluatedEffect::Contrast { amount } => {
                    rgb = rgb.map(|channel| (channel - 128.0) * amount + 128.0);
                }
                EvaluatedEffect::Saturation { amount } => {
                    let luma = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
                    rgb = rgb.map(|channel| luma * (1.0 - amount) + channel * amount);
                }
                EvaluatedEffect::Tint { colour, amount } => {
                    let amount = amount.clamp(0.0, 1.0);
                    rgb = std::array::from_fn(|channel| {
                        rgb[channel] * (1.0 - amount) + f64::from(colour[channel]) * amount
                    });
                }
                _ => {}
            }
        }
        rgb
    }
    #[test]
    fn alpha_composition_is_known() {
        assert_eq!(
            source_over(Rgba([0, 0, 255, 255]), Rgba([255, 0, 0, 128]), 1.0),
            Rgba([128, 0, 127, 255])
        );
    }
    #[test]
    fn bilinear_sampling_blends_four_neighbors() {
        let mut image = RgbaImage::new(2, 2);
        image.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        image.put_pixel(1, 0, Rgba([100, 0, 0, 255]));
        image.put_pixel(0, 1, Rgba([0, 100, 0, 255]));
        image.put_pixel(1, 1, Rgba([100, 100, 0, 255]));
        assert_eq!(sample_bilinear(&image, 1.0, 1.0), Rgba([50, 50, 0, 255]));
    }

    #[test]
    fn basic_color_effects_apply_in_declared_order() {
        let transform = ColourTransform::from_effects([
            EvaluatedEffect::Brightness { amount: 0.1 },
            EvaluatedEffect::Tint {
                colour: [0, 0, 255, 255],
                amount: 0.5,
            },
        ]);
        assert_eq!(
            apply_colour_transform(Rgba([100, 0, 0, 255]), transform),
            Rgba([63, 13, 140, 255])
        );
    }

    #[test]
    fn saturation_zero_produces_neutral_channels() {
        let pixel = apply_colour_transform(
            Rgba([255, 0, 0, 255]),
            ColourTransform::from_effects([EvaluatedEffect::Saturation { amount: 0.0 }]),
        );
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }

    #[test]
    fn contrast_one_is_identity() {
        assert_eq!(
            apply_colour_transform(
                Rgba([30, 140, 250, 180]),
                ColourTransform::from_effects([EvaluatedEffect::Contrast { amount: 1.0 }])
            ),
            Rgba([30, 140, 250, 180])
        );
    }

    #[test]
    fn combined_colour_matrix_matches_ordered_sequential_effects() {
        let effects = [
            EvaluatedEffect::Brightness { amount: -0.12 },
            EvaluatedEffect::Saturation { amount: 0.55 },
            EvaluatedEffect::Contrast { amount: 1.15 },
            EvaluatedEffect::Tint {
                colour: [30, 120, 240, 255],
                amount: 0.3,
            },
        ];
        let input = Rgba([180, 80, 40, 173]);
        let expected = apply_sequential(
            [
                f64::from(input[0]),
                f64::from(input[1]),
                f64::from(input[2]),
            ],
            &effects,
        )
        .map(|channel| channel.round().clamp(0.0, 255.0) as u8);
        let actual = apply_colour_transform(input, ColourTransform::from_effects(effects));
        for channel in 0..3 {
            assert!(actual[channel].abs_diff(expected[channel]) <= 1);
        }
        assert_eq!(actual[3], input[3]);
    }

    #[test]
    fn identity_colour_matrix_leaves_pixels_unchanged() {
        let pixel = Rgba([31, 127, 249, 90]);
        assert_eq!(
            apply_colour_transform(pixel, ColourTransform::default()),
            pixel
        );
    }

    #[test]
    fn inverse_affine_matches_transform_reference_mapping() {
        let transform = Transform2D {
            position: crate::domain::Point { x: 0.37, y: 0.61 },
            anchor: crate::domain::Point { x: 0.4, y: 0.7 },
            scale: crate::domain::Point { x: 1.3, y: 0.8 },
            rotation_radians: 0.42,
        };
        let inverse = InverseAffine::for_transform(transform, 320, 180, 140.0, 90.0);
        let mapped = inverse.map(81.5, 44.5);
        let reference = transform.destination_to_source(81.5, 44.5, 320, 180, 140, 90);
        assert!((mapped.x - reference.x).abs() < 1e-10 && (mapped.y - reference.y).abs() < 1e-10);
    }

    #[test]
    fn blend_modes_are_distinct_and_preserve_alpha() {
        let destination = Rgba([40, 100, 200, 255]);
        let source = Rgba([200, 80, 20, 255]);
        let add = blend_pixel(destination, source, crate::project::BlendMode::Add, 1.0);
        let multiply = blend_pixel(
            destination,
            source,
            crate::project::BlendMode::Multiply,
            1.0,
        );
        let screen = blend_pixel(destination, source, crate::project::BlendMode::Screen, 1.0);
        let overlay = blend_pixel(destination, source, crate::project::BlendMode::Overlay, 1.0);
        assert_eq!(add[3], 255);
        assert_eq!(multiply[3], 255);
        assert_ne!(add, multiply);
        assert_ne!(screen, overlay);
    }

    #[test]
    fn zero_radius_blur_is_an_exact_noop() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, Rgba([255, 0, 0, 127]));
        source.put_pixel(1, 0, Rgba([0, 0, 255, 255]));
        let mut target = RgbaImage::new(2, 1);
        blur(&source, &mut target, 0.0, None);
        assert_eq!(source, target);
    }

    #[test]
    fn chromatic_zero_amount_is_an_exact_noop() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([17, 83, 201, 129]));
        let mut target = RgbaImage::new(2, 2);
        chromatic(&source, &mut target, 0.0, 0.0);
        assert_eq!(source, target);
    }

    #[test]
    fn rotated_scanline_advances_after_an_out_of_bounds_sample() {
        let source = RgbaImage::from_pixel(4, 2, Rgba([255, 0, 0, 255]));
        let transform = Transform2D {
            position: crate::domain::Point { x: 0.5, y: 0.5 },
            anchor: crate::domain::Point { x: 0.5, y: 0.5 },
            scale: crate::domain::Point { x: 1.0, y: 1.0 },
            rotation_radians: 0.7,
        };
        let (min_x, max_x, min_y, max_y) = visible_bounds(transform, 4.0, 2.0, 12, 12);
        let inverse = InverseAffine::for_transform(transform, 12, 12, 4.0, 2.0);
        let entering_row = (min_y..max_y)
            .find(|y| {
                let first = inverse.map(f64::from(min_x) + 0.5, f64::from(*y) + 0.5);
                let later_is_valid = (min_x + 1..max_x).any(|x| {
                    let mapped = inverse.map(f64::from(x) + 0.5, f64::from(*y) + 0.5);
                    mapped.x >= 0.0 && mapped.y >= 0.0 && mapped.x < 4.0 && mapped.y < 2.0
                });
                (first.x < 0.0 || first.y < 0.0 || first.x >= 4.0 || first.y >= 2.0)
                    && later_is_valid
            })
            .expect("rotation has a scanline that enters the source");
        let mut canvas = RgbaImage::new(12, 12);
        draw_image(
            &mut canvas,
            &source,
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            4.0,
            2.0,
            transform,
            1.0,
            ColourTransform::default(),
        );
        assert!(
            (min_x..max_x).any(|x| canvas.get_pixel(x, entering_row)[3] > 0),
            "the scanline must render after its mapping enters the rotated source"
        );
    }

    #[test]
    fn positive_and_negative_rotations_keep_a_non_square_source_visible() {
        let source = RgbaImage::from_fn(5, 3, |x, y| Rgba([x as u8 * 40, y as u8 * 80, 255, 255]));
        for rotation_radians in [0.65, -0.65] {
            let mut canvas = RgbaImage::new(16, 16);
            draw_image(
                &mut canvas,
                &source,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                5.0,
                3.0,
                Transform2D {
                    position: crate::domain::Point { x: 0.5, y: 0.5 },
                    anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                    scale: crate::domain::Point { x: 1.0, y: 1.0 },
                    rotation_radians,
                },
                1.0,
                ColourTransform::default(),
            );
            assert!(canvas.pixels().any(|pixel| pixel[3] > 0));
        }
    }

    #[test]
    fn scale_expands_coverage_around_the_anchor() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        let coverage = |scale| {
            let mut canvas = RgbaImage::new(12, 12);
            draw_image(
                &mut canvas,
                &source,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                2.0,
                2.0,
                Transform2D {
                    position: crate::domain::Point { x: 0.5, y: 0.5 },
                    anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                    scale: crate::domain::Point { x: scale, y: scale },
                    rotation_radians: 0.0,
                },
                1.0,
                ColourTransform::default(),
            );
            canvas.pixels().filter(|pixel| pixel[3] > 0).count()
        };
        assert!(coverage(2.0) > coverage(1.0));
    }

    #[test]
    fn opacity_animation_uses_source_over_alpha() {
        let destination = Rgba([0, 0, 255, 255]);
        let source = Rgba([255, 0, 0, 255]);
        assert_eq!(source_over(destination, source, 0.0), destination);
        assert_eq!(source_over(destination, source, 1.0), source);
        assert_eq!(
            source_over(destination, source, 0.5),
            Rgba([128, 0, 128, 255])
        );
    }
}
