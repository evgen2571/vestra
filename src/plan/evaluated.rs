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
    pub(crate) evaluated_effect_count: usize,
    /// Ordered colour effects collapsed once per layer/frame. The compositor
    /// therefore performs no effect dispatch in its pixel loop.
    pub(crate) colour_transform: ColourTransform,
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

/// An affine RGB operation in byte colour space: `matrix * rgb + offset`.
#[derive(Clone, Copy, Debug)]
pub struct ColourTransform {
    pub(crate) matrix: [[f64; 3]; 3],
    pub(crate) offset: [f64; 3],
}

impl Default for ColourTransform {
    fn default() -> Self {
        Self {
            matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            offset: [0.0; 3],
        }
    }
}

impl ColourTransform {
    fn then(mut self, matrix: [[f64; 3]; 3], offset: [f64; 3]) -> Self {
        let previous_matrix = self.matrix;
        let previous_offset = self.offset;
        self.matrix = multiply(matrix, previous_matrix);
        self.offset = add(multiply_vector(matrix, previous_offset), offset);
        self
    }

    #[must_use]
    pub(crate) fn from_effects(effects: impl IntoIterator<Item = EvaluatedEffect>) -> Self {
        effects
            .into_iter()
            .fold(Self::default(), |transform, effect| match effect {
                EvaluatedEffect::Brightness { amount } => transform.then(
                    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    [amount * 255.0; 3],
                ),
                EvaluatedEffect::Contrast { amount } => transform.then(
                    [[amount, 0.0, 0.0], [0.0, amount, 0.0], [0.0, 0.0, amount]],
                    [128.0 * (1.0 - amount); 3],
                ),
                EvaluatedEffect::Saturation { amount } => {
                    let luma = [0.2126, 0.7152, 0.0722];
                    let matrix = std::array::from_fn(|row| {
                        std::array::from_fn(|column| {
                            luma[column] * (1.0 - amount) + if row == column { amount } else { 0.0 }
                        })
                    });
                    transform.then(matrix, [0.0; 3])
                }
                EvaluatedEffect::Tint { colour, amount } => {
                    let amount = amount.clamp(0.0, 1.0);
                    transform.then(
                        [
                            [1.0 - amount, 0.0, 0.0],
                            [0.0, 1.0 - amount, 0.0],
                            [0.0, 0.0, 1.0 - amount],
                        ],
                        [
                            f64::from(colour[0]) * amount,
                            f64::from(colour[1]) * amount,
                            f64::from(colour[2]) * amount,
                        ],
                    )
                }
            })
    }
}

fn multiply(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|index| left[row][index] * right[index][column])
                .sum()
        })
    })
}

fn multiply_vector(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|row| {
        (0..3)
            .map(|column| matrix[row][column] * vector[column])
            .sum()
    })
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| left[index] + right[index])
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
                evaluated_effect_count: layer.effects.len(),
                colour_transform: ColourTransform::from_effects(
                    layer
                        .effects
                        .iter()
                        .map(|effect| evaluate_effect(effect, relative)),
                ),
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
