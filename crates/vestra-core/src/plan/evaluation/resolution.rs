//! Adapt authored coordinates before rasterization, retaining effect stages.

use super::{
    EvaluatedComposition, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedMaskInput,
    EvaluatedSource,
};
use crate::{
    animation::Transform2D,
    domain::Point,
    plan::{Canvas, ColourTransform, CompiledSizing, EvaluationError, TemporalDependency},
};

pub(super) fn apply(
    frame: &mut EvaluatedFrame,
    canvas: &Canvas,
    dependency: TemporalDependency,
) -> Result<(), EvaluationError> {
    if (canvas.width, canvas.height) == (canvas.logical_width, canvas.logical_height) {
        return Ok(());
    }
    let scale = canvas.render_scale();
    let extent = canvas.fitted_extent();
    scale_layers(
        &mut frame.layers,
        (canvas.logical_width, canvas.logical_height),
        extent,
        scale,
    )?;
    for effect in &mut frame.post_effects {
        scale_effect(effect, scale)?;
        let center = match effect {
            EvaluatedEffect::ZoomBlur { anchor, .. } => Some(anchor),
            EvaluatedEffect::RadialBlur { center, .. } => Some(center),
            _ => None,
        };
        if let Some(center) = center {
            center.x = (center.x * f64::from(extent.0) + f64::from(canvas.width - extent.0) / 2.0)
                / f64::from(canvas.width);
            center.y = (center.y * f64::from(extent.1) + f64::from(canvas.height - extent.1) / 2.0)
                / f64::from(canvas.height);
        }
    }
    if extent != (canvas.width, canvas.height) {
        if frame.background[3] > 0
            && frame.background[3] < 255
            && frame
                .layers
                .iter()
                .any(|layer| layer.visible && layer.blend_mode != crate::project::BlendMode::Normal)
        {
            return Err(EvaluationError::ResolutionEffectLimit(
                "letterboxing with a partially transparent background and non-normal root blending is unsupported",
            ));
        }
        // The fitted canvas is rendered at output density, never at authored resolution.
        let mut background = EvaluatedLayer {
            compiled_layer_index: usize::MAX - 1,
            visible: true,
            content_dependency: TemporalDependency::Static,
            source: EvaluatedSource::SolidColor {
                colour: frame.background,
            },
            transform: Transform2D::identity(Point { x: 0.5, y: 0.5 }, Point { x: 0.5, y: 0.5 }),
            opacity: 1.0,
            effects: Vec::new(),
            masks: Vec::new(),
            matte: None,
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        };
        let mut layers = Vec::with_capacity(frame.layers.len() + 1);
        if frame.background[3] == 255 {
            layers.push(background.clone());
        }
        layers.append(&mut frame.layers);
        background.compiled_layer_index = usize::MAX;
        background.content_dependency = dependency;
        background.source = EvaluatedSource::Group {
            composition: EvaluatedComposition {
                is_sized: true,
                width: extent.0,
                height: extent.1,
                layers,
            },
        };
        frame.layers = vec![background];
    }
    Ok(())
}

fn scaled_extent((width, height): (u32, u32), scale: f64) -> (u32, u32) {
    (
        (f64::from(width) * scale).round().max(1.0) as u32,
        (f64::from(height) * scale).round().max(1.0) as u32,
    )
}

fn scale_position(transform: &mut Transform2D, old: (u32, u32), new: (u32, u32), scale: f64) {
    transform.position.x *= f64::from(old.0) * scale / f64::from(new.0);
    transform.position.y *= f64::from(old.1) * scale / f64::from(new.1);
}

fn scale_anchor(transform: &mut Transform2D, old: (u32, u32), new: (u32, u32), scale: f64) {
    transform.anchor.x *= f64::from(old.0) * scale / f64::from(new.0);
    transform.anchor.y *= f64::from(old.1) * scale / f64::from(new.1);
}

fn scale_layers(
    layers: &mut [EvaluatedLayer],
    old: (u32, u32),
    new: (u32, u32),
    scale: f64,
) -> Result<(), EvaluationError> {
    for layer in layers {
        let owner_extent = match &layer.source {
            EvaluatedSource::Group { composition } => (composition.width, composition.height),
            _ => old,
        };
        let scaled_owner = scaled_extent(owner_extent, scale);
        let mut owner_transform = layer.transform;
        scale_position(&mut owner_transform, old, new, scale);
        scale_anchor(&mut owner_transform, owner_extent, scaled_owner, scale);
        scale_source(&mut layer.source, &mut layer.transform, old, new, scale)?;
        for effect in &mut layer.effects {
            scale_effect(effect, scale)?;
        }
        for mask in &mut layer.masks {
            mask.owner_transform = Some(owner_transform);
            mask.feather *= scale as f32;
            if mask.feather > 256.0 {
                return Err(EvaluationError::ResolutionEffectLimit(
                    "mask feather exceeds 256 output pixels",
                ));
            }
            match &mut mask.input {
                EvaluatedMaskInput::Source { source, .. } => {
                    scale_source(
                        source,
                        &mut mask.transform,
                        owner_extent,
                        scaled_owner,
                        scale,
                    )?;
                }
                _ => {
                    scale_position(&mut mask.transform, owner_extent, scaled_owner, scale);
                    mask.transform.scale.x *= scale;
                    mask.transform.scale.y *= scale;
                }
            }
        }
    }
    Ok(())
}

fn scale_source(
    source: &mut EvaluatedSource,
    transform: &mut Transform2D,
    old: (u32, u32),
    new: (u32, u32),
    scale: f64,
) -> Result<(), EvaluationError> {
    scale_position(transform, old, new, scale);
    match source {
        EvaluatedSource::Group { composition } => {
            let local_old = (composition.width, composition.height);
            let local_new = scaled_extent(local_old, scale);
            scale_layers(&mut composition.layers, local_old, local_new, scale)?;
            scale_anchor(transform, local_old, local_new, scale);
            composition.width = local_new.0;
            composition.height = local_new.1;
        }
        EvaluatedSource::Image { sizing, .. }
        | EvaluatedSource::Video { sizing, .. }
        | EvaluatedSource::Shape { sizing, .. } => {
            *sizing = match sizing {
                CompiledSizing::Fit => CompiledSizing::FitTo {
                    width: old.0,
                    height: old.1,
                },
                CompiledSizing::Cover => CompiledSizing::CoverTo {
                    width: old.0,
                    height: old.1,
                },
                _ => sizing.clone(),
            };
            transform.scale.x *= scale;
            transform.scale.y *= scale;
        }
        EvaluatedSource::Text { .. } => {
            transform.scale.x *= scale;
            transform.scale.y *= scale;
        }
        EvaluatedSource::Spectrum2D {
            x,
            y,
            width,
            height,
            ..
        } => {
            let horizontal = f64::from(old.0) * scale / f64::from(new.0);
            let vertical = f64::from(old.1) * scale / f64::from(new.1);
            *x *= horizontal;
            *width *= horizontal;
            *y *= vertical;
            *height *= vertical;
        }
        EvaluatedSource::SolidColor { .. } | EvaluatedSource::ParticleSystem { .. } => {}
    }
    Ok(())
}

fn scale_effect(effect: &mut EvaluatedEffect, scale: f64) -> Result<(), EvaluationError> {
    if effect.is_identity() {
        return Ok(());
    }
    match effect {
        EvaluatedEffect::GaussianBlur { radius }
        | EvaluatedEffect::DirectionalBlur { radius, .. }
        | EvaluatedEffect::Glow { radius, .. }
        | EvaluatedEffect::Bloom { radius, .. }
        | EvaluatedEffect::Sharpen { radius, .. } => {
            *radius *= scale;
            if *radius > 32.0 {
                return Err(EvaluationError::ResolutionEffectLimit(
                    "blur radius exceeds 32 output pixels",
                ));
            }
        }
        EvaluatedEffect::MotionBlur {
            radius, max_radius, ..
        } => {
            *radius *= scale;
            *max_radius *= scale;
            if *radius > 32.0 {
                return Err(EvaluationError::ResolutionEffectLimit(
                    "motion blur radius exceeds 32 output pixels",
                ));
            }
        }
        EvaluatedEffect::ZoomBlur { radius, .. } => *radius *= scale,
        EvaluatedEffect::RadialBlur { amount, .. }
        | EvaluatedEffect::ChromaticAberration { amount, .. } => *amount *= scale,
        EvaluatedEffect::Ascii { parameters } => {
            parameters.cell_width =
                (f64::from(parameters.cell_width) * scale).round().max(1.0) as u32;
            parameters.cell_height =
                (f64::from(parameters.cell_height) * scale).round().max(1.0) as u32;
        }
        EvaluatedEffect::Halftone { cell_size, .. } => *cell_size = (*cell_size * scale).max(1.0),
        EvaluatedEffect::PixelSort { segment_length, .. } => {
            let scaled = (f64::from(*segment_length) * scale).round().max(1.0);
            if scaled > 256.0 {
                return Err(EvaluationError::ResolutionEffectLimit(
                    "pixel sort segment exceeds 256 output pixels",
                ));
            }
            *segment_length = scaled as u16;
        }
        EvaluatedEffect::Crt {
            scanline_spacing,
            mask_spacing,
            jitter,
            ..
        } => {
            *scanline_spacing *= scale;
            let spacing = (f64::from(*mask_spacing) * scale).round().max(1.0);
            if spacing > f64::from(u8::MAX) {
                return Err(EvaluationError::ResolutionEffectLimit(
                    "CRT mask spacing exceeds 255 output pixels",
                ));
            }
            *mask_spacing = spacing as u8;
            *jitter *= scale;
            if *jitter > 128.0 {
                return Err(EvaluationError::ResolutionEffectLimit(
                    "CRT jitter exceeds 128 output pixels",
                ));
            }
        }
        EvaluatedEffect::PaletteMap {
            input_detail,
            input_detail_radius,
            input_scale,
            ..
        }
        | EvaluatedEffect::OrderedDither {
            input_detail,
            input_detail_radius,
            input_scale,
            ..
        } => {
            *input_detail_radius *= scale;
            // Unit analysis is the identity, including when the raster is enlarged.
            if *input_scale > 1.0 {
                *input_scale = (*input_scale * scale).round().max(1.0);
            }
            if (*input_detail > 0.0 && *input_detail_radius > 32.0) || *input_scale > 256.0 {
                return Err(EvaluationError::ResolutionEffectLimit(
                    "palette input detail or analysis exceeds output pixel limits",
                ));
            }
            if let EvaluatedEffect::OrderedDither {
                scale: pattern_scale,
                ..
            } = effect
            {
                let scaled = (f64::from(*pattern_scale) * scale).round().max(1.0);
                if scaled > f64::from(u8::MAX) {
                    return Err(EvaluationError::ResolutionEffectLimit(
                        "dither scale exceeds 255 output pixels",
                    ));
                }
                *pattern_scale = scaled as u8;
            }
        }
        // Normalized parameters and source-space Motion Tile are resolution independent.
        EvaluatedEffect::ColourTransform { .. }
        | EvaluatedEffect::Brightness { .. }
        | EvaluatedEffect::Contrast { .. }
        | EvaluatedEffect::Saturation { .. }
        | EvaluatedEffect::Tint { .. }
        | EvaluatedEffect::MotionTile { .. }
        | EvaluatedEffect::Vignette { .. }
        | EvaluatedEffect::ColorAdjust { .. }
        | EvaluatedEffect::CameraShake { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_effect_sizes_preserve_source_tile_units_and_pixel_blur_units() {
        let mut radial = EvaluatedEffect::RadialBlur {
            amount: 16.0,
            center: Point { x: 0.3, y: 0.7 },
        };
        scale_effect(&mut radial, 0.5).unwrap();
        assert!(matches!(
            radial,
            EvaluatedEffect::RadialBlur { amount: 8.0, .. }
        ));
        let mut tile = EvaluatedEffect::MotionTile {
            output_width_percent: 300.0,
            output_height_percent: 200.0,
            tile_center: Point { x: 0.25, y: 0.75 },
            mirror_edges: true,
        };
        scale_effect(&mut tile, 0.5).unwrap();
        assert!(matches!(
            tile,
            EvaluatedEffect::MotionTile {
                output_width_percent: 300.0,
                output_height_percent: 200.0,
                ..
            }
        ));
        let mut blur = EvaluatedEffect::GaussianBlur { radius: 8.0 };
        scale_effect(&mut blur, 2.0).unwrap();
        assert!(matches!(
            blur,
            EvaluatedEffect::GaussianBlur { radius: 16.0 }
        ));
        assert!(scale_effect(&mut blur, 4.0).is_err());
    }

    #[test]
    fn upscale_rejects_integer_effect_overflow_before_narrowing() {
        let mut sort = EvaluatedEffect::PixelSort {
            lower_threshold: 0.0,
            upper_threshold: 1.0,
            amount: 1.0,
            direction: crate::project::PixelSortDirection::Horizontal,
            order: crate::project::PixelSortOrder::Ascending,
            segment_length: 256,
        };
        assert!(scale_effect(&mut sort, 4096.0).is_err());
        let mut crt = EvaluatedEffect::Crt {
            amount: 1.0,
            curvature: 0.0,
            scanline_strength: 0.0,
            scanline_spacing: 2.0,
            mask_strength: 1.0,
            grain: 0.0,
            jitter: 0.0,
            flicker: 0.0,
            rolling_strength: 0.0,
            rolling_width: 0.1,
            phase: 0.0,
            mask_spacing: 6,
            seed: 0,
        };
        assert!(scale_effect(&mut crt, 256.0).is_err());
    }

    #[test]
    fn letterbox_rejects_only_partial_background_alpha_with_visible_root_blending() {
        for (alpha, height, visible, rejected) in [
            (128, 64, true, true),
            (128, 32, true, false),
            (0, 64, true, false),
            (255, 64, true, false),
            (128, 64, false, false),
        ] {
            let canvas = Canvas {
                width: 64,
                height,
                logical_width: 128,
                logical_height: 64,
                background: [32, 64, 96, alpha],
                preview: false,
            };
            let mut frame = EvaluatedFrame {
                width: 64,
                height,
                background: canvas.background,
                time: 0,
                post_effects: Vec::new(),
                evaluated_track_count: 0,
                layers: vec![EvaluatedLayer {
                    compiled_layer_index: 0,
                    visible,
                    content_dependency: TemporalDependency::Static,
                    source: EvaluatedSource::SolidColor {
                        colour: [255, 0, 0, 255],
                    },
                    transform: Transform2D::identity(
                        Point { x: 0.5, y: 0.5 },
                        Point { x: 0.5, y: 0.5 },
                    ),
                    opacity: 1.0,
                    effects: Vec::new(),
                    masks: Vec::new(),
                    matte: None,
                    colour_transform: ColourTransform::default(),
                    blend_mode: crate::project::BlendMode::Multiply,
                }],
            };
            let result = apply(&mut frame, &canvas, TemporalDependency::Static);
            assert_eq!(
                result.is_err(),
                rejected,
                "alpha={alpha}, height={height}, visible={visible}"
            );
            if let Err(error) = result {
                assert!(
                    error
                        .to_string()
                        .contains("partially transparent background and non-normal root blending")
                );
            }
        }
    }

    #[test]
    fn global_blur_center_maps_to_the_fitted_content_viewport() {
        let canvas = Canvas {
            width: 64,
            height: 64,
            logical_width: 128,
            logical_height: 64,
            background: [0, 0, 0, 255],
            preview: false,
        };
        let mut frame = EvaluatedFrame {
            width: 64,
            height: 64,
            background: canvas.background,
            time: 0,
            layers: Vec::new(),
            evaluated_track_count: 0,
            post_effects: vec![EvaluatedEffect::ZoomBlur {
                radius: 8.0,
                samples: 8,
                anchor: Point { x: 0.25, y: 0.75 },
                direction: crate::project::ZoomBlurDirection::Centered,
            }],
        };
        apply(&mut frame, &canvas, TemporalDependency::Static).unwrap();
        assert!(matches!(
            frame.post_effects[0],
            EvaluatedEffect::ZoomBlur {
                radius: 4.0,
                anchor: Point { x: 0.25, y: 0.625 },
                ..
            }
        ));
        let EvaluatedSource::Group { composition } = &frame.layers[0].source else {
            panic!("fitted group")
        };
        assert_eq!((composition.width, composition.height), (64, 32));
    }
}
