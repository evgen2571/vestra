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

pub use colour::ColourTransform;
pub use effects::EvaluatedEffect;

#[derive(Clone, Debug)]
pub struct EvaluatedFrame {
    pub time: u128,
    pub background: [u8; 4],
    pub width: u32,
    pub height: u32,
    pub layers: Vec<EvaluatedLayer>,
    pub post_effects: Vec<EvaluatedEffect>,
    pub evaluated_track_count: u64,
}

#[derive(Clone, Debug)]
pub struct EvaluatedLayer {
    pub source: EvaluatedSource,
    pub opacity: f64,
    /// Ordered local effect chain. Image effects consume and produce complete
    /// surfaces, so its order is never inferred or rearranged by a backend.
    pub effects: Vec<EvaluatedEffect>,
    /// Legacy single-pass representation used by the basic WGPU path. Advanced
    /// chains are rejected by that backend before frame rendering.
    pub colour_transform: ColourTransform,
    pub blend_mode: crate::project::BlendMode,
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
pub fn evaluate(plan: &RenderPlan, active: &[ScheduledItem], time: u128) -> EvaluatedFrame {
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
                    crate::camera_shake::apply(
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
