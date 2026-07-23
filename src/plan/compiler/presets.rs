use crate::{
    Diagnostic,
    animation::{Interpolation, Keyframe, Track},
    domain::Point,
    plan::{CompilationStats, CompiledEffect, CompiledLayer, TimedEffect, TransformContribution},
    project::Preset,
};

use super::to_nanos;

pub(super) fn apply(
    layer: &mut CompiledLayer,
    preset: &Preset,
    duration: f64,
    compilation: &mut CompilationStats,
) -> Result<(), Diagnostic> {
    let timing = preset.timing();
    let start = to_nanos(timing.start, &layer.id)?;
    let interval_duration = timing
        .duration
        .unwrap_or_else(|| default_duration(preset, duration - timing.start));
    let end = start.saturating_add(to_nanos(interval_duration, &layer.id)?);
    let span = end - start;
    let key = |time, value| Keyframe {
        time,
        value,
        interpolation: Interpolation::EaseInOut,
    };
    let mut generated = Vec::new();
    let add_shake = |effects: &mut Vec<CompiledEffect>, intensity: f64, seed: u64| {
        effects.push(CompiledEffect::CameraShake {
            position_amount: Track::new(0.012 * intensity),
            rotation_degrees: Track::new(1.2 * intensity),
            scale_amount: Track::new(0.01 * intensity),
            frequency: Track::new(14.0),
            seed,
            attack: 0.03,
            decay: 0.22,
        });
    };
    match preset {
        Preset::SlowDrift { intensity, .. } => {
            let mut contribution = TransformContribution::identity();
            contribution.start = start;
            contribution.end = end;
            contribution.position_offset = Track {
                base_value: Point {
                    x: -0.01 * intensity,
                    y: 0.008 * intensity,
                },
                keyframes: vec![key(
                    end,
                    Point {
                        x: 0.01 * intensity,
                        y: -0.008 * intensity,
                    },
                )],
            };
            contribution.scale_multiplier = Track {
                base_value: Point { x: 1.0, y: 1.0 },
                keyframes: vec![key(
                    end,
                    Point {
                        x: 1.0 + 0.04 * intensity,
                        y: 1.0 + 0.04 * intensity,
                    },
                )],
            };
            layer.transform_contributions.push(contribution);
        }
        Preset::ZoomPunch { intensity, .. } => {
            let peak = start + span / 4;
            let mut contribution = TransformContribution::identity();
            contribution.start = start;
            contribution.end = start + span / 2;
            contribution.scale_multiplier = Track {
                base_value: Point { x: 1.0, y: 1.0 },
                keyframes: vec![
                    key(
                        peak,
                        Point {
                            x: 1.0 + 0.16 * intensity,
                            y: 1.0 + 0.16 * intensity,
                        },
                    ),
                    key(start + span / 2, Point { x: 1.0, y: 1.0 }),
                ],
            };
            layer.transform_contributions.push(contribution);
        }
        Preset::Impact {
            intensity, seed, ..
        } => {
            add_zoom_punch(layer, start, span, *intensity, 0.20);
            add_shake(&mut generated, *intensity, *seed);
            generated.push(CompiledEffect::ChromaticAberration {
                amount: Track {
                    base_value: 0.0,
                    keyframes: vec![
                        Keyframe {
                            time: span / 8,
                            value: 3.0 * intensity,
                            interpolation: Interpolation::EaseInOut,
                        },
                        Keyframe {
                            time: span / 3,
                            value: 0.0,
                            interpolation: Interpolation::EaseInOut,
                        },
                    ],
                },
                angle_degrees: Track::new(0.0),
            });
            generated.push(pulse_tint(span, *intensity));
        }
        Preset::HeavyImpact {
            intensity, seed, ..
        } => {
            add_zoom_punch(layer, start, span, *intensity, 0.28);
            add_shake(&mut generated, *intensity * 1.8, *seed);
            generated.push(CompiledEffect::DirectionalBlur {
                radius: Track {
                    base_value: 0.0,
                    keyframes: vec![
                        Keyframe {
                            time: span / 8,
                            value: 10.0 * intensity,
                            interpolation: Interpolation::EaseInOut,
                        },
                        Keyframe {
                            time: span / 3,
                            value: 0.0,
                            interpolation: Interpolation::EaseInOut,
                        },
                    ],
                },
                angle_degrees: Track::new(0.0),
            });
            generated.push(CompiledEffect::ChromaticAberration {
                amount: pulse_track(span, 5.0 * intensity),
                angle_degrees: Track::new(0.0),
            });
            generated.push(pulse_tint(span, *intensity * 0.75));
        }
        Preset::FocusReveal { intensity, .. } => {
            let mut contribution = TransformContribution::identity();
            contribution.start = start;
            contribution.end = start + span / 2;
            contribution.scale_multiplier = Track {
                base_value: Point {
                    x: 1.0 + 0.04 * intensity,
                    y: 1.0 + 0.04 * intensity,
                },
                keyframes: vec![key(start + span / 2, Point { x: 1.0, y: 1.0 })],
            };
            layer.transform_contributions.push(contribution);
            generated.push(CompiledEffect::GaussianBlur {
                radius: Track {
                    base_value: 8.0 * intensity,
                    keyframes: vec![Keyframe {
                        time: span / 2,
                        value: 0.0,
                        interpolation: Interpolation::EaseInOut,
                    }],
                },
            });
            generated.push(CompiledEffect::Sharpen {
                amount: Track {
                    base_value: 0.0,
                    keyframes: vec![Keyframe {
                        time: span / 2,
                        value: 0.35 * intensity,
                        interpolation: Interpolation::EaseInOut,
                    }],
                },
                radius: Track::new(1.0),
            });
        }
    }
    compilation.generated_local_effect_count += generated.len();
    layer.effects.splice(
        0..0,
        generated
            .into_iter()
            .map(|effect| TimedEffect { start, end, effect }),
    );
    Ok(())
}

fn default_duration(preset: &Preset, remaining: f64) -> f64 {
    let preferred = match preset {
        Preset::SlowDrift { .. } => remaining,
        Preset::ZoomPunch { .. } => 0.35,
        Preset::Impact { .. } => 0.28,
        Preset::HeavyImpact { .. } => 0.4,
        Preset::FocusReveal { .. } => 0.8,
    };
    preferred.min(remaining)
}

fn add_zoom_punch(layer: &mut CompiledLayer, start: u128, span: u128, intensity: f64, amount: f64) {
    let mut contribution = TransformContribution::identity();
    contribution.start = start;
    contribution.end = start + span / 2;
    contribution.scale_multiplier = Track {
        base_value: Point { x: 1.0, y: 1.0 },
        keyframes: vec![
            Keyframe {
                time: start + span / 8,
                value: Point {
                    x: 1.0 + amount * intensity,
                    y: 1.0 + amount * intensity,
                },
                interpolation: Interpolation::EaseOut,
            },
            Keyframe {
                time: start + span / 2,
                value: Point { x: 1.0, y: 1.0 },
                interpolation: Interpolation::EaseInOut,
            },
        ],
    };
    layer.transform_contributions.push(contribution);
}

fn pulse_track(end: u128, amount: f64) -> Track<f64> {
    Track {
        base_value: 0.0,
        keyframes: vec![
            Keyframe {
                time: end / 8,
                value: amount,
                interpolation: Interpolation::EaseOut,
            },
            Keyframe {
                time: end / 3,
                value: 0.0,
                interpolation: Interpolation::EaseInOut,
            },
        ],
    }
}

fn pulse_tint(end: u128, intensity: f64) -> CompiledEffect {
    CompiledEffect::Tint {
        colour: [255, 255, 255, 255],
        amount: pulse_track(end, (0.25 * intensity).clamp(0.0, 1.0)),
    }
}
