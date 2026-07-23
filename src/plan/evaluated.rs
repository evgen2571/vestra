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

#[derive(Clone, Debug)]
pub enum EvaluatedEffect {
    Brightness {
        amount: f64,
    },
    Contrast {
        amount: f64,
    },
    Saturation {
        amount: f64,
    },
    Tint {
        colour: [u8; 4],
        amount: f64,
    },
    GaussianBlur {
        radius: f64,
    },
    DirectionalBlur {
        radius: f64,
        angle_degrees: f64,
    },
    Glow {
        threshold: f64,
        radius: f64,
        intensity: f64,
        colour: [u8; 4],
    },
    ChromaticAberration {
        amount: f64,
        angle_degrees: f64,
    },
    Vignette {
        amount: f64,
        radius: f64,
        softness: f64,
        colour: [u8; 4],
    },
    Sharpen {
        amount: f64,
        radius: f64,
    },
    ColorAdjust {
        exposure: f64,
        gamma: f64,
        black_point: f64,
        white_point: f64,
    },
    CameraShake {
        position_amount: f64,
        rotation_radians: f64,
        scale_amount: f64,
        frequency: f64,
        seed: u64,
        attack: f64,
        decay: f64,
    },
    MotionBlur {
        radius: f64,
        angle_degrees: f64,
        intensity: f64,
        shutter_angle: f64,
        max_radius: f64,
        samples: u8,
    },
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
                _ => transform,
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
                    transform: evaluate_transform(layer, relative, &mut evaluated_track_count),
                }
            }
            CompiledVisualSource::SolidColor { colour } => {
                EvaluatedSource::SolidColor { colour: *colour }
            }
        };
        let mut effects = layer
            .effects
            .iter()
            .map(|effect| {
                evaluated_track_count += 1;
                evaluate_effect(effect, relative)
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
                let before = relative.saturating_sub(half_window);
                let after = relative.saturating_add(half_window);
                let mut ignored_tracks = 0;
                let start = evaluate_transform(layer, before, &mut ignored_tracks).position;
                let end = evaluate_transform(layer, after, &mut ignored_tracks).position;
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
                    position_amount,
                    rotation_radians,
                    scale_amount,
                    frequency,
                    seed,
                    attack,
                    decay,
                } = effect
                {
                    apply_camera_shake(
                        transform,
                        relative,
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
            .map(|effect| evaluate_effect(effect, time))
            .collect(),
        evaluated_track_count,
    }
}

fn evaluate_transform(
    layer: &crate::plan::CompiledLayer,
    relative: u128,
    evaluated_track_count: &mut u64,
) -> Transform2D {
    *evaluated_track_count += 4;
    let mut transform = Transform2D {
        position: layer.transform.position.evaluate(relative),
        anchor: layer.transform.anchor.evaluate(relative),
        scale: layer.transform.scale.evaluate(relative),
        rotation_radians: layer.transform.rotation_radians.evaluate(relative),
    };
    for contribution in &layer.transform_contributions {
        if relative < contribution.start || relative > contribution.end {
            continue;
        }
        *evaluated_track_count += 3;
        let position = contribution.position_offset.evaluate(relative);
        let scale = contribution.scale_multiplier.evaluate(relative);
        transform.position.x += position.x;
        transform.position.y += position.y;
        transform.scale.x *= scale.x;
        transform.scale.y *= scale.y;
        transform.rotation_radians += contribution.rotation_radians_offset.evaluate(relative);
    }
    transform
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
        CompiledEffect::GaussianBlur { radius } => EvaluatedEffect::GaussianBlur {
            radius: radius.evaluate(time),
        },
        CompiledEffect::DirectionalBlur {
            radius,
            angle_degrees,
        } => EvaluatedEffect::DirectionalBlur {
            radius: radius.evaluate(time),
            angle_degrees: angle_degrees.evaluate(time),
        },
        CompiledEffect::Glow {
            threshold,
            radius,
            intensity,
            colour,
        } => EvaluatedEffect::Glow {
            threshold: threshold.evaluate(time),
            radius: radius.evaluate(time),
            intensity: intensity.evaluate(time),
            colour: *colour,
        },
        CompiledEffect::ChromaticAberration {
            amount,
            angle_degrees,
        } => EvaluatedEffect::ChromaticAberration {
            amount: amount.evaluate(time),
            angle_degrees: angle_degrees.evaluate(time),
        },
        CompiledEffect::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => EvaluatedEffect::Vignette {
            amount: amount.evaluate(time),
            radius: radius.evaluate(time),
            softness: softness.evaluate(time),
            colour: *colour,
        },
        CompiledEffect::Sharpen { amount, radius } => EvaluatedEffect::Sharpen {
            amount: amount.evaluate(time),
            radius: radius.evaluate(time),
        },
        CompiledEffect::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => EvaluatedEffect::ColorAdjust {
            exposure: exposure.evaluate(time),
            gamma: gamma.evaluate(time),
            black_point: black_point.evaluate(time),
            white_point: white_point.evaluate(time),
        },
        CompiledEffect::CameraShake {
            position_amount,
            rotation_degrees,
            scale_amount,
            frequency,
            seed,
            attack,
            decay,
        } => EvaluatedEffect::CameraShake {
            position_amount: position_amount.evaluate(time),
            rotation_radians: rotation_degrees.evaluate(time).to_radians(),
            scale_amount: scale_amount.evaluate(time),
            frequency: frequency.evaluate(time),
            seed: *seed,
            attack: *attack,
            decay: *decay,
        },
        CompiledEffect::MotionBlur {
            intensity,
            shutter_angle,
            max_radius,
            samples,
        } => EvaluatedEffect::MotionBlur {
            radius: 0.0,
            angle_degrees: 0.0,
            intensity: intensity.evaluate(time),
            shutter_angle: shutter_angle.evaluate(time),
            max_radius: max_radius.evaluate(time),
            samples: *samples,
        },
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the evaluated shake signal is kept allocation-free"
)]
fn apply_camera_shake(
    transform: &mut Transform2D,
    time: u128,
    position_amount: f64,
    rotation_radians: f64,
    scale_amount: f64,
    frequency: f64,
    seed: u64,
    attack: f64,
    decay: f64,
) {
    let seconds = time as f64 / 1_000_000_000.0;
    let attack = if attack <= 0.0 {
        1.0
    } else {
        (seconds / attack).clamp(0.0, 1.0)
    };
    let envelope = attack * (-seconds / decay.max(0.000_1)).exp();
    let phase = |channel| {
        let value = stable_seed(seed.wrapping_add(channel));
        f64::from((value >> 11) as u32) / f64::from(u32::MAX) * std::f64::consts::TAU
    };
    let sample = |channel: u64| {
        let primary = phase(channel);
        let secondary = phase(channel.wrapping_add(0x9e37_79b9));
        ((seconds * frequency * std::f64::consts::TAU + primary).sin()
            + 0.5 * (seconds * frequency * 1.618 * std::f64::consts::TAU + secondary).sin())
            / 1.5
    };
    transform.position.x += sample(0) * position_amount * envelope;
    transform.position.y += sample(1) * position_amount * envelope;
    transform.rotation_radians += sample(2) * rotation_radians * envelope;
    let scale = 1.0 + sample(3).abs() * scale_amount * envelope;
    transform.scale.x *= scale;
    transform.scale.y *= scale;
}

fn stable_seed(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
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
        apply_camera_shake(
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
        apply_camera_shake(
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
        apply_camera_shake(
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
        apply_camera_shake(
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
        apply_camera_shake(&mut next, 100_000_000, 0.02, 0.1, 0.01, 14.0, 8, 0.03, 0.22);
        assert!((first.position.x - next.position.x).abs() > 0.000_1);
        assert!((first.position.y - next.position.y).abs() > 0.000_1);
    }
}
