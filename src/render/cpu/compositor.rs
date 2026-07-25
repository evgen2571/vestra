use image::{Rgba, RgbaImage};

use crate::plan::{ColourTransform, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer};
use crate::render::{
    blend::blend_surface,
    cpu::{assets::PreparedAssets, effects, raster::draw_layer},
};

pub(crate) use super::surfaces::EffectSurfacePool;

#[cfg(test)]
use crate::render::cpu::effects::blur;
#[cfg(test)]
use crate::render::cpu::raster::{
    apply_colour_transform, draw_image, sample_bilinear, sample_edge, visible_bounds,
};
#[cfg(test)]
use crate::{animation::Transform2D, domain::Crop, plan::EvaluatedSource, render::geometry};
#[cfg(test)]
use image::GenericImage;

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
        if uses_direct_colour_path(layer) {
            draw_layer(canvas, assets, layer, layer.opacity, layer.colour_transform);
            continue;
        }
        surfaces.clear();
        draw_layer(
            surfaces.current(),
            assets,
            layer,
            1.0,
            ColourTransform::default(),
        );
        effects::apply_chain(surfaces, &layer.effects);
        blend_surface(canvas, surfaces.current(), layer.blend_mode, layer.opacity);
    }
    effects::apply_to(surfaces, canvas, &frame.post_effects);
}

fn uses_direct_colour_path(layer: &EvaluatedLayer) -> bool {
    matches!(layer.blend_mode, crate::project::BlendMode::Normal)
        && layer
            .effects
            .iter()
            .all(EvaluatedEffect::is_basic_colour_effect)
}

/// Samples along the ray through each pixel instead of applying a spatial
/// Gaussian. Radius is expressed in pixels and converted to a bounded scale
/// exposure around the centre of the source surface.
#[cfg(test)]
fn zoom_blur(
    source: &RgbaImage,
    target: &mut RgbaImage,
    radius: f64,
    samples: u8,
    anchor: crate::domain::Point,
    direction: crate::project::ZoomBlurDirection,
) {
    if radius <= 0.01 {
        target.copy_from(source, 0, 0).expect("same dimensions");
        return;
    }
    let amount = (radius / f64::from(source.width().max(source.height()))).clamp(0.0, 0.5);
    let samples = i32::from(samples);
    let centre_x = anchor.x * (f64::from(source.width()) - 1.0);
    let centre_y = anchor.y * (f64::from(source.height()) - 1.0);
    for (x, y, _) in source.enumerate_pixels() {
        let ray_x = f64::from(x) - centre_x;
        let ray_y = f64::from(y) - centre_y;
        let mut premultiplied = [0.0; 3];
        let mut alpha = 0.0;
        for index in 0..samples {
            let unit = f64::from(index) / f64::from(samples - 1);
            let exposure = match direction {
                crate::project::ZoomBlurDirection::Inward => -unit,
                crate::project::ZoomBlurDirection::Outward => unit,
                crate::project::ZoomBlurDirection::Centered => unit * 2.0 - 1.0,
            };
            let scale = 1.0 + exposure * amount;
            let pixel = sample_edge(
                source,
                centre_x + ray_x * scale + 0.5,
                centre_y + ray_y * scale + 0.5,
            );
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::EvaluatedEffect;
    use crate::render::blend::{blend_pixel, source_over};
    use crate::render::cpu::effects::effect_pass_plan;

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
    fn directional_blur_keeps_transparent_edges_coloured() {
        let mut source = RgbaImage::from_pixel(7, 1, Rgba([0, 0, 255, 0]));
        source.put_pixel(3, 0, Rgba([255, 128, 32, 255]));
        let mut target = RgbaImage::new(7, 1);
        blur(&source, &mut target, 2.0, Some(0.0), Some(5));
        let edge = target.get_pixel(2, 0);
        assert!(edge[3] > 0);
        assert!(edge[0] > edge[2]);
    }

    #[test]
    fn directional_blur_matches_the_transparent_pixel_golden_fixture() {
        let mut source = RgbaImage::new(5, 1);
        source.put_pixel(2, 0, Rgba([255, 80, 20, 192]));
        let mut output = RgbaImage::new(5, 1);
        blur(&source, &mut output, 1.0, Some(0.0), Some(3));
        assert_eq!(
            output.as_raw(),
            &[
                0, 0, 0, 0, 255, 80, 20, 64, 255, 80, 20, 64, 255, 80, 20, 64, 0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn zoom_blur_streaks_along_the_ray_from_the_anchor() {
        let mut source = RgbaImage::new(9, 9);
        source.put_pixel(7, 4, Rgba([255, 255, 255, 255]));
        let mut target = RgbaImage::new(9, 9);
        zoom_blur(
            &source,
            &mut target,
            4.0,
            12,
            crate::domain::Point { x: 0.5, y: 0.5 },
            crate::project::ZoomBlurDirection::Centered,
        );
        assert!(target.get_pixel(6, 4)[3] > target.get_pixel(7, 3)[3]);
    }

    #[test]
    fn vignette_normalizes_each_frame_axis_independently() {
        let source = RgbaImage::from_pixel(10, 100, Rgba([255, 255, 255, 255]));
        let mut target = RgbaImage::new(10, 100);
        crate::render::cpu::vignette::apply(&source, &mut target, 1.0, 0.0, 1.0, [0, 0, 0, 255]);
        let top = target.get_pixel(5, 0)[0];
        let side = target.get_pixel(0, 50)[0];
        assert!(top.abs_diff(side) <= 32);
        assert!(target.get_pixel(5, 50)[0] > top);
    }

    #[test]
    fn vertical_vignette_matches_the_pixel_golden_fixture() {
        let source = RgbaImage::from_pixel(3, 5, Rgba([200, 160, 120, 255]));
        let mut target = RgbaImage::new(3, 5);
        crate::render::cpu::vignette::apply(&source, &mut target, 0.8, 0.25, 0.5, [0, 0, 0, 255]);
        assert_eq!(
            target.as_raw(),
            &[
                40, 32, 24, 255, 40, 32, 24, 255, 40, 32, 24, 255, 40, 32, 24, 255, 152, 122, 91,
                255, 40, 32, 24, 255, 67, 53, 40, 255, 200, 160, 120, 255, 67, 53, 40, 255, 40, 32,
                24, 255, 152, 122, 91, 255, 40, 32, 24, 255, 40, 32, 24, 255, 40, 32, 24, 255, 40,
                32, 24, 255,
            ]
        );
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
    fn basic_colour_effects_keep_the_direct_render_path() {
        let base = EvaluatedLayer {
            source: EvaluatedSource::SolidColor {
                colour: [0, 0, 0, 255],
            },
            opacity: 1.0,
            effects: vec![EvaluatedEffect::Brightness { amount: 0.1 }],
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        };
        assert!(uses_direct_colour_path(&base));
        let mut advanced = base;
        advanced.effects = vec![EvaluatedEffect::GaussianBlur { radius: 1.0 }];
        assert!(!uses_direct_colour_path(&advanced));
        advanced.effects.clear();
        advanced.blend_mode = crate::project::BlendMode::Screen;
        assert!(!uses_direct_colour_path(&advanced));
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
    fn evaluated_effect_identity_plan_skips_only_identity_work() {
        assert!(effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 0.0 }).is_empty());
        assert!(
            effect_pass_plan(&EvaluatedEffect::ColorAdjust {
                exposure: 0.0,
                gamma: 1.0,
                black_point: 0.0,
                white_point: 1.0,
            })
            .is_empty()
        );
        assert!(
            effect_pass_plan(&EvaluatedEffect::CameraShake {
                local_time: 0,
                position_amount: 1.0,
                rotation_radians: 1.0,
                scale_amount: 1.0,
                frequency: 1.0,
                seed: 1,
                attack: 0.0,
                decay: 0.0,
            })
            .is_empty()
        );
        assert!(
            !effect_pass_plan(&EvaluatedEffect::Glow {
                threshold: 0.5,
                radius: 2.0,
                intensity: 1.0,
                colour: [255, 255, 255, 255],
            })
            .is_empty()
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
        let inverse = geometry::InverseAffine::for_transform(transform, 320, 180, 140.0, 90.0);
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
        blur(&source, &mut target, 0.0, None, None);
        assert_eq!(source, target);
    }

    #[test]
    fn chromatic_zero_amount_is_an_exact_noop() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([17, 83, 201, 129]));
        let mut target = RgbaImage::new(2, 2);
        crate::render::cpu::chromatic::apply(&source, &mut target, 0.0, 0.0);
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
        let inverse = geometry::InverseAffine::for_transform(transform, 12, 12, 4.0, 2.0);
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
