//! Immutable per-frame program shared by every render backend.

use crate::{
    animation::Transform2D,
    domain::Crop,
    plan::{CompiledEffect, CompiledSizing, CompiledVisualSource, RenderPlan, ScheduledItem},
};

#[derive(Clone, Debug)]
pub struct EvaluatedFrame {
    pub(crate) time: u128,
    pub(crate) background: [u8; 4],
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) layers: Vec<EvaluatedLayer>,
}

#[derive(Clone, Debug)]
pub struct EvaluatedLayer {
    pub(crate) source: EvaluatedSource,
    pub(crate) transform: Transform2D,
    pub(crate) opacity: f64,
    pub(crate) effects: Vec<EvaluatedEffect>,
}

#[derive(Clone, Debug)]
pub enum EvaluatedSource {
    Image {
        asset_index: usize,
        crop: Crop,
        sizing: CompiledSizing,
        cacheable_crop: bool,
    },
    SolidColor {
        colour: [u8; 4],
    },
}

#[derive(Clone, Debug)]
pub enum EvaluatedEffect {
    Brightness { amount: f64 },
    Contrast { amount: f64 },
    Saturation { amount: f64 },
    Tint { colour: [u8; 4], amount: f64 },
}

#[must_use]
pub(crate) fn evaluate(plan: &RenderPlan, active: &[ScheduledItem], time: u128) -> EvaluatedFrame {
    let layers = active
        .iter()
        .filter_map(|ScheduledItem(index)| {
            let layer = &plan.layers[*index];
            let relative = time.saturating_sub(layer.start_nanos);
            let opacity = layer
                .opacity_contributions
                .iter()
                .fold(layer.opacity.evaluate(relative), |value, track| {
                    value * track.evaluate(relative)
                })
                .clamp(0.0, 1.0);
            (opacity > 0.0).then(|| EvaluatedLayer {
                source: match &layer.source {
                    CompiledVisualSource::Image {
                        asset_index,
                        crop,
                        sizing,
                        cacheable_crop,
                    } => EvaluatedSource::Image {
                        asset_index: *asset_index,
                        crop: crop.evaluate(relative),
                        sizing: sizing.clone(),
                        cacheable_crop: *cacheable_crop,
                    },
                    CompiledVisualSource::SolidColor { colour } => {
                        EvaluatedSource::SolidColor { colour: *colour }
                    }
                },
                transform: Transform2D {
                    position: layer.transform.position.evaluate(relative),
                    anchor: layer.transform.anchor.evaluate(relative),
                    scale: layer.transform.scale.evaluate(relative),
                    rotation_radians: layer.transform.rotation_radians.evaluate(relative),
                },
                opacity,
                effects: layer
                    .effects
                    .iter()
                    .map(|effect| evaluate_effect(effect, relative))
                    .collect(),
            })
        })
        .collect();
    EvaluatedFrame {
        time,
        background: plan.canvas.background,
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers,
    }
}

fn evaluate_effect(effect: &CompiledEffect, time: u128) -> EvaluatedEffect {
    match effect {
        CompiledEffect::Brightness { amount } => EvaluatedEffect::Brightness {
            amount: amount.evaluate(time),
        },
        CompiledEffect::Contrast { amount } => EvaluatedEffect::Contrast {
            amount: amount.evaluate(time),
        },
        CompiledEffect::Saturation { amount } => EvaluatedEffect::Saturation {
            amount: amount.evaluate(time),
        },
        CompiledEffect::Tint { colour, amount } => EvaluatedEffect::Tint {
            colour: *colour,
            amount: amount.evaluate(time),
        },
    }
}
