//! Immutable per-frame program shared by every render backend.

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{
        ColourTransform, CompiledSizing, CompiledVisualSource, EvaluationContext, EvaluationError,
        RenderPlan, ScheduledItem, TemporalDependency,
    },
};

mod colour;
mod effects;
mod motion;
mod transform;

pub use effects::EvaluatedEffect;
/// Workspace-internal raw effect evaluation used by renderer conformance tests.
/// The stable `video-editor` SDK does not re-export this execution helper.
pub use effects::evaluate as evaluate_effect;

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
    /// Stable index in the immutable compiled plan. Render caches use this
    /// plan-local identity, never a frame number.
    pub compiled_layer_index: usize,
    /// Compiler-owned cacheability proof. Renderers consume it without
    /// reclassifying tracks or effect parameters.
    pub content_dependency: TemporalDependency,
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
    Spectrum2D {
        bands: Vec<f32>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        bar_gap_ratio: f64,
        colour: [u8; 4],
    },
}

/// Evaluates a plan with no prepared procedural resources.
///
/// Normal rendering uses [`evaluate_with_context`] so a missing runtime signal
/// remains a diagnosable error instead of an implicit fallback.
pub fn evaluate(
    plan: &RenderPlan,
    active: &[ScheduledItem],
    project_time: u128,
) -> Result<EvaluatedFrame, EvaluationError> {
    let signals = crate::plan::PreparedScalarSignals::empty();
    evaluate_with_context(
        plan,
        active,
        project_time,
        &EvaluationContext::new(&signals),
    )
}

/// Evaluates a plan using explicitly supplied immutable runtime resources.
pub fn evaluate_with_context(
    plan: &RenderPlan,
    active: &[ScheduledItem],
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedFrame, EvaluationError> {
    let mut layers = Vec::with_capacity(active.len());
    let mut evaluated_track_count = 0;
    for ScheduledItem(index) in active {
        let layer = &plan.layers[*index];
        let relative = project_time.saturating_sub(layer.start_nanos);
        let mut opacity = layer.opacity.evaluate(relative, project_time, context)?;
        evaluated_track_count += 1;
        for track in &layer.opacity_contributions {
            opacity *= track.evaluate(relative);
            evaluated_track_count += 1;
        }
        if !opacity.is_finite() {
            return Err(EvaluationError::NonFiniteScalarProperty);
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
                    transform: transform::evaluate(
                        layer,
                        relative,
                        project_time,
                        context,
                        &mut evaluated_track_count,
                    )?,
                }
            }
            CompiledVisualSource::SolidColor { colour } => {
                EvaluatedSource::SolidColor { colour: *colour }
            }
            CompiledVisualSource::Spectrum2D {
                band_signals,
                x,
                y,
                width,
                height,
                bar_gap_ratio,
                colour,
            } => EvaluatedSource::Spectrum2D {
                bands: band_signals
                    .iter()
                    .map(|signal| context.sample_scalar(*signal, project_time))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(|amplitude| amplitude.clamp(0.0, 1.0) as f32)
                    .collect(),
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                bar_gap_ratio: *bar_gap_ratio,
                colour: *colour,
            },
        };
        let mut effects = layer
            .effects
            .iter()
            .filter(|effect| effect.active_at(relative))
            .map(|effect| {
                evaluated_track_count += 1;
                effects::evaluate(
                    &effect.effect,
                    relative - effect.start,
                    project_time,
                    context,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
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
                let before_project_time = layer.start_nanos.saturating_add(before);
                let after_project_time = layer.start_nanos.saturating_add(after);
                let start = transform::evaluate(
                    layer,
                    before,
                    before_project_time,
                    context,
                    &mut ignored_tracks,
                )?
                .position;
                let end = transform::evaluate(
                    layer,
                    after,
                    after_project_time,
                    context,
                    &mut ignored_tracks,
                )?
                .position;
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
            compiled_layer_index: *index,
            content_dependency: layer.content_dependency,
            source,
            opacity,
            colour_transform: ColourTransform::from_effects(effects.clone()),
            effects,
            blend_mode: layer.blend_mode,
        });
    }
    Ok(EvaluatedFrame {
        time: project_time,
        background: plan.canvas.background,
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers,
        post_effects: plan
            .post_effects
            .iter()
            .filter(|effect| effect.active_at(project_time))
            .map(|effect| {
                effects::evaluate(
                    &effect.effect,
                    project_time - effect.start,
                    project_time,
                    context,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        evaluated_track_count,
    })
}
