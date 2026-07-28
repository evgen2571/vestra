//! Immutable per-frame program shared by every render backend.

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{CompiledSizing, CompiledVisualSource, RenderPlan, ScheduledItem},
};

mod colour;
mod effects;
mod motion;
mod transform;

/// Temporary compatibility facade for core-owned camera shake evaluation.
mod shake {
    pub use video_editor_core::camera_shake::apply;
}

pub use colour::ColourTransform;
pub use effects::EvaluatedEffect;

#[derive(Clone, Debug)]
pub struct EvaluatedFrame {
    pub(crate) time: u128,
    pub(crate) background: [u8; 4],
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) layers: Vec<EvaluatedLayer>,
    pub(crate) post_effects: Vec<EvaluatedEffect>,
    pub(crate) evaluated_track_count: u64,
}

#[derive(Clone, Debug)]
pub struct EvaluatedLayer {
    pub(crate) source: EvaluatedSource,
    pub(crate) opacity: f64,
    /// Ordered local effect chain. Image effects consume and produce complete
    /// surfaces, so its order is never inferred or rearranged by a backend.
    pub(crate) effects: Vec<EvaluatedEffect>,
    /// Legacy single-pass representation used by the basic WGPU path. Advanced
    /// chains are rejected by that backend before frame rendering.
    pub(crate) colour_transform: ColourTransform,
    pub(crate) blend_mode: crate::project::BlendMode,
}

#[derive(Clone, Debug)]
pub enum EvaluatedSource {
    Image {
        asset_index: usize,
        crop: Crop,
        sizing: CompiledSizing,
        cacheable_crop: bool,
        transform: Transform2D,
    },
    SolidColor {
        colour: [u8; 4],
    },
}

#[must_use]
pub(crate) fn evaluate(plan: &RenderPlan, active: &[ScheduledItem], time: u128) -> EvaluatedFrame {
    let mut layers = Vec::with_capacity(active.len());
    let mut evaluated_track_count = 0;
    for ScheduledItem(index) in active {
        let layer = &plan.layers[*index];
        let relative = time.saturating_sub(layer.start_nanos);
        let mut opacity = layer.opacity.evaluate(relative);
        evaluated_track_count += 1;
        for track in &layer.opacity_contributions {
            opacity *= track.evaluate(relative);
            evaluated_track_count += 1;
        }
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            continue;
        }
        let mut source = match &layer.source {
            CompiledVisualSource::Image {
                asset_index,
                crop,
                sizing,
                cacheable_crop,
            } => {
                evaluated_track_count += 1;
                EvaluatedSource::Image {
                    asset_index: *asset_index,
                    crop: crop.evaluate(relative),
                    sizing: sizing.clone(),
                    cacheable_crop: *cacheable_crop,
                    transform: transform::evaluate(layer, relative, &mut evaluated_track_count),
                }
            }
            CompiledVisualSource::SolidColor { colour } => {
                EvaluatedSource::SolidColor { colour: *colour }
            }
        };
        let mut effects = layer
            .effects
            .iter()
            .filter(|effect| effect.active_at(relative))
            .map(|effect| {
                evaluated_track_count += 1;
                effects::evaluate(&effect.effect, relative - effect.start)
            })
            .collect::<Vec<_>>();
        for effect in &mut effects {
            if let EvaluatedEffect::MotionBlur {
                radius,
                angle_degrees,
                intensity,
                shutter_angle,
                max_radius,
                ..
            } = effect
            {
                let frame_duration = (1_000_000_000_u128 * u128::from(plan.frame_rate.1))
                    / u128::from(plan.frame_rate.0);
                let exposure = (frame_duration as f64 * (*shutter_angle / 360.0)).round() as u128;
                let half_window = exposure / 2;
                let (lower, upper) = motion::sample_bounds(layer, relative);
                let before = relative.saturating_sub(half_window).max(lower);
                let after = relative.saturating_add(half_window).min(upper);
                let mut ignored_tracks = 0;
                let start = transform::evaluate(layer, before, &mut ignored_tracks).position;
                let end = transform::evaluate(layer, after, &mut ignored_tracks).position;
                let dx = (end.x - start.x) * f64::from(plan.canvas.width);
                let dy = (end.y - start.y) * f64::from(plan.canvas.height);
                let displacement = (dx * dx + dy * dy).sqrt();
                if displacement <= 0.000_1 || exposure == 0 {
                    *radius = 0.0;
                } else {
                    *angle_degrees = dy.atan2(dx).to_degrees();
                    *radius = (displacement * *intensity).clamp(0.0, *max_radius);
                }
            }
        }
        if let EvaluatedSource::Image { transform, .. } = &mut source {
            for effect in &effects {
                if let EvaluatedEffect::CameraShake {
                    local_time,
                    position_amount,
                    rotation_radians,
                    scale_amount,
                    frequency,
                    seed,
                    attack,
                    decay,
                } = effect
                {
                    shake::apply(
                        transform,
                        *local_time,
                        *position_amount,
                        *rotation_radians,
                        *scale_amount,
                        *frequency,
                        *seed,
                        *attack,
                        *decay,
                    );
                }
            }
        }
        layers.push(EvaluatedLayer {
            source,
            opacity,
            colour_transform: ColourTransform::from_effects(effects.clone()),
            effects,
            blend_mode: layer.blend_mode,
        });
    }
    EvaluatedFrame {
        time,
        background: plan.canvas.background,
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers,
        post_effects: plan
            .post_effects
            .iter()
            .filter(|effect| effect.active_at(time))
            .map(|effect| effects::evaluate(&effect.effect, time - effect.start))
            .collect(),
        evaluated_track_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        animation::Transform2D,
        plan::{CompileOptions, compile},
        project::{ValidationOptions, load_and_validate},
    };

    fn canonical_plan() -> RenderPlan {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("valid project");
        compile(&validated, CompileOptions::default()).expect("plan")
    }

    #[test]
    fn solid_layers_skip_transform_tracks_while_images_evaluate_them() {
        let plan = canonical_plan();
        let flash = plan
            .layers
            .iter()
            .position(|layer| matches!(layer.source, CompiledVisualSource::SolidColor { .. }))
            .expect("flash layer");
        let flash_frame = evaluate(&plan, &[ScheduledItem(flash)], 1_150_000_000);
        assert_eq!(flash_frame.evaluated_track_count, 1);
        assert!(matches!(
            flash_frame.layers[0].source,
            EvaluatedSource::SolidColor { .. }
        ));

        let image = plan
            .layers
            .iter()
            .position(|layer| matches!(layer.source, CompiledVisualSource::Image { .. }))
            .expect("image layer");
        let image_frame = evaluate(&plan, &[ScheduledItem(image)], 100_000_000);
        // Opacity, one transition contribution, crop, four transform tracks,
        // and saturation are all evaluated for this image layer.
        assert_eq!(image_frame.evaluated_track_count, 8);
        assert!(matches!(
            image_frame.layers[0].source,
            EvaluatedSource::Image { .. }
        ));
    }

    #[test]
    fn camera_shake_is_deterministic_and_continuous() {
        let base = Transform2D {
            position: crate::domain::Point { x: 0.5, y: 0.5 },
            anchor: crate::domain::Point { x: 0.5, y: 0.5 },
            scale: crate::domain::Point { x: 1.0, y: 1.0 },
            rotation_radians: 0.0,
        };
        let mut first = base;
        let mut repeated = base;
        let mut nearby = base;
        super::shake::apply(
            &mut first,
            100_000_000,
            0.02,
            0.1,
            0.01,
            14.0,
            7,
            0.03,
            0.22,
        );
        super::shake::apply(
            &mut repeated,
            100_000_000,
            0.02,
            0.1,
            0.01,
            14.0,
            7,
            0.03,
            0.22,
        );
        super::shake::apply(
            &mut nearby,
            101_000_000,
            0.02,
            0.1,
            0.01,
            14.0,
            7,
            0.03,
            0.22,
        );
        assert_eq!(first.position.x, repeated.position.x);
        assert!((first.position.x - nearby.position.x).abs() < 0.02);
    }

    #[test]
    fn camera_shake_seeds_produce_distinct_continuous_patterns() {
        let base = Transform2D {
            position: crate::domain::Point { x: 0.5, y: 0.5 },
            anchor: crate::domain::Point { x: 0.5, y: 0.5 },
            scale: crate::domain::Point { x: 1.0, y: 1.0 },
            rotation_radians: 0.0,
        };
        let mut first = base;
        let mut next = base;
        super::shake::apply(
            &mut first,
            100_000_000,
            0.02,
            0.1,
            0.01,
            14.0,
            7,
            0.03,
            0.22,
        );
        super::shake::apply(&mut next, 100_000_000, 0.02, 0.1, 0.01, 14.0, 8, 0.03, 0.22);
        assert!((first.position.x - next.position.x).abs() > 0.000_1);
        assert!((first.position.y - next.position.y).abs() > 0.000_1);
    }

    #[test]
    fn motion_samples_stay_inside_an_active_generated_contribution() {
        let validated = load_and_validate(
            std::path::Path::new("examples/transitions/directional-push.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("valid directional-push example");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        let layer = plan
            .layers
            .iter()
            .find(|layer| !layer.transform_contributions.is_empty())
            .expect("canonical transition creates a contribution");
        let contribution = &layer.transform_contributions[0];
        assert_eq!(
            super::motion::sample_bounds(layer, contribution.start),
            (contribution.start, contribution.end)
        );
        assert_eq!(
            super::motion::sample_bounds(layer, contribution.end),
            (0, layer.duration_nanos)
        );
    }
}
