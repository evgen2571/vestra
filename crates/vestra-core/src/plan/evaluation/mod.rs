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
/// The stable `vestra` SDK does not re-export this execution helper.
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
    /// Frame-local Layer presentation state. Source variants contain only
    /// source-local content and evaluation data.
    pub transform: Transform2D,
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
pub struct RasterPresentation {
    pub crop: Crop,
    pub sizing: CompiledSizing,
    pub cacheable_crop: bool,
}

#[derive(Clone, Debug)]
pub enum EvaluatedSource {
    Image {
        asset_index: usize,
        crop: Crop,
        sizing: CompiledSizing,
        cacheable_crop: bool,
    },
    Video {
        asset_index: usize,
        source_index: usize,
        source_time: f64,
        crop: Crop,
        sizing: CompiledSizing,
    },
    SolidColor {
        colour: [u8; 4],
    },
    Shape {
        shape_index: usize,
        sizing: CompiledSizing,
    },
    Text {
        text_index: usize,
    },
    Spectrum2D {
        bands: Vec<f32>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        bar_gap_ratio: f64,
        min_bar_height_ratio: f64,
        layout: crate::project::Spectrum2DLayout,
        gradient: Option<(
            crate::project::Spectrum2DGradientDirection,
            [u8; 4],
            [u8; 4],
        )>,
        colour: [u8; 4],
    },
    /// Compact renderer-independent particle state. Particle reconstruction
    /// and lifetime styling remain lazy in the shared core iterator.
    ParticleSystem {
        system: std::sync::Arc<crate::plan::CompiledParticleSystem>,
        time_nanos: u128,
        appearance: crate::plan::EvaluatedParticleAppearance,
    },
    Group {
        composition: EvaluatedComposition,
    },
}

impl EvaluatedSource {
    #[must_use]
    pub fn raster_presentation(&self) -> Option<RasterPresentation> {
        match self {
            Self::Image {
                crop,
                sizing,
                cacheable_crop,
                ..
            } => Some(RasterPresentation {
                crop: *crop,
                sizing: sizing.clone(),
                cacheable_crop: *cacheable_crop,
            }),
            Self::Video { crop, sizing, .. } => Some(RasterPresentation {
                crop: *crop,
                sizing: sizing.clone(),
                cacheable_crop: false,
            }),
            Self::Shape { sizing, .. } => Some(RasterPresentation {
                crop: Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                sizing: sizing.clone(),
                cacheable_crop: false,
            }),
            Self::Text { .. } => Some(RasterPresentation {
                crop: Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                sizing: CompiledSizing::Original,
                cacheable_crop: false,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EvaluatedComposition {
    pub layers: Vec<EvaluatedLayer>,
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
    let (layers, evaluated_track_count) = evaluate_layers(
        &plan.layers,
        active,
        project_time,
        project_time,
        plan.frame_rate,
        plan.canvas.width,
        plan.canvas.height,
        true,
        plan.images.len() + plan.shapes.len() + plan.texts.len(),
        plan.video_slot_stride(),
        context,
    )?;
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

#[allow(
    clippy::too_many_arguments,
    reason = "the recursive evaluator keeps local time, root time, and immutable frame context explicit"
)]
fn evaluate_layers(
    compiled_layers: &[crate::plan::CompiledLayer],
    active: &[ScheduledItem],
    composition_time: u128,
    root_project_time: u128,
    frame_rate: (u64, u64),
    width: u32,
    height: u32,
    root_composition: bool,
    raster_source_base: usize,
    video_slot_stride: usize,
    context: &EvaluationContext<'_>,
) -> Result<(Vec<EvaluatedLayer>, u64), EvaluationError> {
    let mut layers = Vec::with_capacity(active.len());
    let mut evaluated_track_count = 0;
    for ScheduledItem(index) in active {
        let layer = &compiled_layers[*index];
        let relative = composition_time.saturating_sub(layer.start_nanos);
        let mut opacity = layer
            .opacity
            .evaluate(relative, root_project_time, context)?;
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
        let mut transform = transform::evaluate(
            layer,
            relative,
            root_project_time,
            context,
            &mut evaluated_track_count,
        )?;
        let source = match &layer.source {
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
                }
            }
            CompiledVisualSource::SolidColor { colour } => {
                EvaluatedSource::SolidColor { colour: *colour }
            }
            CompiledVisualSource::Video {
                asset_index,
                source_start,
                playback_rate,
                crop,
                sizing,
            } => EvaluatedSource::Video {
                asset_index: *asset_index,
                source_index: raster_source_base
                    + asset_index.saturating_mul(video_slot_stride)
                    + layer.compiled_identity,
                source_time: *source_start + (relative as f64 / 1_000_000_000.0) * *playback_rate,
                crop: *crop,
                sizing: sizing.clone(),
            },
            CompiledVisualSource::Shape { shape_index } => EvaluatedSource::Shape {
                shape_index: *shape_index,
                sizing: CompiledSizing::Original,
            },
            CompiledVisualSource::Text { text_index } => EvaluatedSource::Text {
                text_index: *text_index,
            },
            CompiledVisualSource::Spectrum2D {
                band_signals,
                x,
                y,
                width,
                height,
                bar_gap_ratio,
                min_bar_height_ratio,
                layout,
                gradient,
                colour,
            } => EvaluatedSource::Spectrum2D {
                bands: band_signals
                    .iter()
                    .map(|signal| context.sample_scalar(*signal, root_project_time))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(|amplitude| amplitude.clamp(0.0, 1.0) as f32)
                    .collect(),
                x: *x,
                y: *y,
                width: *width,
                height: *height,
                bar_gap_ratio: *bar_gap_ratio,
                min_bar_height_ratio: *min_bar_height_ratio,
                layout: layout.clone(),
                gradient: *gradient,
                colour: *colour,
            },
            CompiledVisualSource::ParticleSystem(system) => {
                let appearance =
                    system.evaluate_appearance_at(relative, root_project_time, context)?;
                EvaluatedSource::ParticleSystem {
                    system: system.clone(),
                    time_nanos: relative,
                    appearance,
                }
            }
            CompiledVisualSource::Group(composition) => {
                let nested_active = composition
                    .schedule
                    .active_at_time(&composition.layers, relative);
                let (nested_layers, nested_count) = evaluate_layers(
                    &composition.layers,
                    &nested_active,
                    relative,
                    root_project_time,
                    frame_rate,
                    width,
                    height,
                    false,
                    raster_source_base,
                    video_slot_stride,
                    context,
                )?;
                evaluated_track_count += nested_count;
                EvaluatedSource::Group {
                    composition: EvaluatedComposition {
                        layers: nested_layers,
                    },
                }
            }
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
                    root_project_time,
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
                let frame_duration =
                    (1_000_000_000_u128 * u128::from(frame_rate.1)) / u128::from(frame_rate.0);
                let exposure = (frame_duration as f64 * (*shutter_angle / 360.0)).round() as u128;
                let half_window = exposure / 2;
                let (lower, upper) = motion::sample_bounds(layer, relative);
                let before = relative.saturating_sub(half_window).max(lower);
                let after = relative.saturating_add(half_window).min(upper);
                let mut ignored_tracks = 0;
                let before_project_time = if root_composition {
                    layer.start_nanos.saturating_add(before)
                } else {
                    sample_root_time(root_project_time, relative, before)
                };
                let after_project_time = if root_composition {
                    layer.start_nanos.saturating_add(after)
                } else {
                    sample_root_time(root_project_time, relative, after)
                };
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
                let dx = (end.x - start.x) * f64::from(width);
                let dy = (end.y - start.y) * f64::from(height);
                let displacement = (dx * dx + dy * dy).sqrt();
                if displacement <= 0.000_1 || exposure == 0 {
                    *radius = 0.0;
                } else {
                    *angle_degrees = dy.atan2(dx).to_degrees();
                    *radius = (displacement * *intensity).clamp(0.0, *max_radius);
                }
            }
        }
        if source.supports_direct_transform() {
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
                        &mut transform,
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
            compiled_layer_index: layer.compiled_identity,
            content_dependency: layer.content_dependency,
            source,
            transform,
            opacity,
            colour_transform: ColourTransform::from_effects(effects.clone()),
            effects,
            blend_mode: layer.blend_mode,
        });
    }
    Ok((layers, evaluated_track_count))
}

fn sample_root_time(
    current_root_time: u128,
    current_local_time: u128,
    sample_local_time: u128,
) -> u128 {
    if sample_local_time >= current_local_time {
        current_root_time.saturating_add(sample_local_time - current_local_time)
    } else {
        current_root_time.saturating_sub(current_local_time - sample_local_time)
    }
}

impl EvaluatedSource {
    /// Whether Layer transform presentation is applied directly to this
    /// source. Sources that need an adapter keep that decision centralized.
    #[must_use]
    const fn supports_direct_transform(&self) -> bool {
        matches!(
            self,
            Self::Image { .. } | Self::Shape { .. } | Self::Text { .. } | Self::Group { .. }
        )
    }
}
