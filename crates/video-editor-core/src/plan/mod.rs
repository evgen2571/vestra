//! Deterministic compilation and per-frame evaluation for renderer backends.

mod compiler;
mod effect_passes;
mod evaluation;
mod input;
mod model;
mod scalar_property;
mod schedule;
mod signals;

pub use compiler::{CompileOptions, compile};
pub use effect_passes::{
    CompositeMode, EffectOperation, EffectPass, EffectPassInputs, EffectPassPlan,
    EffectPassRequirements, EffectResource, compiled_effect_pass_requirements, effect_pass_plan,
};
pub use evaluation::{EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource};
pub use evaluation::{evaluate, evaluate_with_context};
pub use input::PlanCompileInput;
pub use model::*;
pub(crate) use scalar_property::ScalarPropertyTarget;
pub use scalar_property::{
    CompiledScalarModifier, CompiledScalarProperty, MIN_POSITIVE_PROPERTY_VALUE,
    ScalarModifierOperation, ScalarPropertyConstraint,
};
pub use schedule::{
    ActiveSchedule, ScheduleAction, ScheduleCursor, ScheduleEvent, sort_active_items,
};
pub(crate) use signals::ScalarSignalInterner;
pub use signals::{
    AudioAnalysisRequirement, AudioAnalysisRequirements, AudioAnalysisTap, AudioFrequencyBand,
    AudioScalarFeature, AudioScalarSignal, AudioSignalContractError, ClampTransform,
    CompiledScalarSignal, CompiledScalarSignals, CompiledSignalTransform, CubicResponseCurve,
    EnvelopeTransform, EvaluationContext, EvaluationError, GainTransform, PreparedScalarSignal,
    PreparedScalarSignalError, PreparedScalarSignals, RawScalarSignal, RemapTransform,
    ScalarSignalId, SignalPreparationError, SignalTransformContractError, prepare_scalar_signals,
    prepare_transformed_scalar_signal,
};

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::{
        ActiveSchedule, CompileOptions, CompiledScalarModifier, CompiledScalarProperty,
        CompiledScalarSignal, CompiledScalarSignals, CompiledSignalTransform, EnvelopeTransform,
        EvaluationContext, PlanCompileInput, PreparedScalarSignal, PreparedScalarSignals,
        RawScalarSignal, ScalarModifierOperation, ScalarPropertyConstraint, ScalarSignalId,
        TimedEffect, compile, evaluate_with_context, prepare_scalar_signals,
    };
    use crate::{
        animation::{
            Interpolation as CompiledInterpolation, Keyframe as CompiledKeyframe,
            Track as CompiledTrack,
        },
        plan::{CompiledEffect, TemporalDependency},
        project::{
            ActiveInterval, AudioAnalysisTap as ProjectAudioAnalysisTap,
            AudioScalarFeature as ProjectAudioScalarFeature, Effect, Interpolation,
            InterpolationName, Keyframe, Preset, Project, ScalarModifier,
            ScalarModifierOperation as ProjectScalarModifierOperation, ScalarSignal,
            ScalarSignalSource, SignalTransform, Track,
        },
        validation::ResourceLimits,
    };

    fn canonical_input() -> PlanCompileInput<'static> {
        let project = Box::leak(Box::new(
            serde_json::from_str::<Project>(include_str!(
                "../../../../examples/projects/animation-effects.json"
            ))
            .expect("fixture project"),
        ));
        let assets = Box::leak(Box::new(BTreeMap::from([
            ("red".to_owned(), PathBuf::from("/resolved/red.png")),
            ("blue".to_owned(), PathBuf::from("/resolved/blue.png")),
        ])));
        let durations = Box::leak(Box::new(BTreeMap::new()));
        PlanCompileInput::new(
            project,
            ResourceLimits::default(),
            std::path::Path::new("/projects"),
            assets,
            durations,
            6.0,
            (24, 1),
            144,
            &[],
        )
    }

    fn compile_project(project: Project) -> super::RenderPlan {
        let project = Box::leak(Box::new(project));
        let assets = Box::leak(Box::new(BTreeMap::from([
            ("red".to_owned(), PathBuf::from("/resolved/red.png")),
            ("blue".to_owned(), PathBuf::from("/resolved/blue.png")),
        ])));
        let durations = Box::leak(Box::new(BTreeMap::new()));
        compile(
            PlanCompileInput::new(
                project,
                ResourceLimits::default(),
                std::path::Path::new("/projects"),
                assets,
                durations,
                6.0,
                (24, 1),
                144,
                &[],
            ),
            CompileOptions::default(),
        )
        .expect("plan")
    }

    fn canonical_project() -> Project {
        serde_json::from_str(include_str!(
            "../../../../examples/projects/animation-effects.json"
        ))
        .expect("fixture project")
    }

    fn stage_layer_from_five_to_six_seconds(layer: &mut super::CompiledLayer) {
        stage_layer(layer, 5.0, 6.0);
    }

    fn stage_layer(layer: &mut super::CompiledLayer, start_seconds: f64, end_seconds: f64) {
        let start = crate::plan_time::to_nanos(start_seconds, "test layer").expect("start time");
        let end = crate::plan_time::to_nanos(end_seconds, "test layer").expect("end time");
        layer.start_nanos = start;
        layer.duration_nanos = end - start;
        layer.start_frame =
            crate::plan_time::first_frame_at_or_after(start, (24, 1)).expect("start frame");
        layer.end_frame =
            crate::plan_time::first_frame_at_or_after(end, (24, 1)).expect("end frame");
        layer.draw_key.start_nanos = start;
    }

    #[test]
    fn canonical_fixture_compiles_from_supplied_preflight_data() {
        let plan = compile(canonical_input(), CompileOptions::default()).expect("plan");
        assert_eq!((plan.canvas.width, plan.canvas.height), (320, 180));
        assert_eq!(plan.images.len(), 2);
        assert_eq!(plan.layers.len(), 3);
        assert_eq!(
            plan.configured_output,
            PathBuf::from("/projects/../output/animation-effects.mp4")
        );
        assert!(plan.scalar_signals.is_empty());
        assert!(plan.audio_analysis_requirements.is_empty());
    }

    #[test]
    fn canonical_inline_signals_deduplicate_complete_and_raw_work() {
        let mut project = canonical_project();
        let signal = ScalarSignal {
            source: ScalarSignalSource::Audio {
                tap: ProjectAudioAnalysisTap::Master,
                feature: ProjectAudioScalarFeature::Rms,
            },
            transforms: vec![SignalTransform::Gain { gain: 2.0 }],
        };
        let alternate = ScalarSignal {
            source: ScalarSignalSource::Audio {
                tap: ProjectAudioAnalysisTap::Master,
                feature: ProjectAudioScalarFeature::Rms,
            },
            transforms: vec![SignalTransform::Gain { gain: 3.0 }],
        };
        let first = &mut project.visual.clips[0];
        first.effects = vec![Effect::Brightness {
            id: "public-modulated-brightness".into(),
            amount: crate::project::ScalarProperty {
                track: Track::constant(0.0),
                modifiers: vec![ScalarModifier {
                    operation: ProjectScalarModifierOperation::Add,
                    signal: signal.clone(),
                }],
            },
        }];
        first.opacity.modifiers = vec![ScalarModifier {
            operation: ProjectScalarModifierOperation::Add,
            signal: signal.clone(),
        }];
        first
            .transform
            .as_mut()
            .expect("image transform")
            .component_modifiers
            .position_x = vec![ScalarModifier {
            operation: ProjectScalarModifierOperation::Add,
            signal,
        }];
        project.visual.clips[1].opacity.modifiers = vec![ScalarModifier {
            operation: ProjectScalarModifierOperation::Add,
            signal: alternate,
        }];
        let plan = compile_project(project);
        assert_eq!(plan.scalar_signals.len(), 2);
        assert_eq!(plan.audio_analysis_requirements.iter().len(), 1);
        assert_eq!(
            plan.layers[0].opacity.modifiers[0].signal,
            plan.layers[0].transform.position_x_modifiers[0].signal
        );
        assert!(matches!(
            plan.layers[0].effects.as_slice(),
            [super::TimedEffect {
                effect: CompiledEffect::Brightness { .. },
                dependency: TemporalDependency::Dynamic,
                ..
            }]
        ));
        assert_eq!(
            plan.layers[0].content_dependency,
            TemporalDependency::Dynamic
        );
    }

    #[test]
    fn schedule_and_evaluation_share_the_compiled_plan() {
        let plan = compile(canonical_input(), CompileOptions::default()).expect("plan");
        let schedule = ActiveSchedule::compile(&plan);
        let mut cursor = schedule.cursor();
        let active = cursor
            .events_at(0)
            .iter()
            .filter_map(|event| {
                (event.action == super::ScheduleAction::Activate).then_some(event.item)
            })
            .collect::<Vec<_>>();
        let signals = PreparedScalarSignals::empty();
        let frame = evaluate_with_context(&plan, &active, 0, &EvaluationContext::new(&signals))
            .expect("frame");
        assert_eq!(frame.layers.len(), 1);
    }

    #[test]
    fn evaluation_applies_scalar_properties_with_project_time_signals() {
        let mut plan = compile(canonical_input(), CompileOptions::default()).expect("plan");
        let layer = &mut plan.layers[0];
        stage_layer_from_five_to_six_seconds(layer);
        layer.opacity.authored_track = CompiledTrack {
            base_value: 0.4,
            keyframes: vec![CompiledKeyframe {
                time: 500_000_000,
                value: 0.8,
                interpolation: CompiledInterpolation::Linear,
            }],
        };
        layer.opacity.modifiers = vec![CompiledScalarModifier {
            operation: ScalarModifierOperation::Add,
            signal: ScalarSignalId::new(0),
        }];
        layer.opacity_contributions = vec![CompiledTrack::new(0.5)];
        layer.effects.push(TimedEffect {
            start: 0,
            end: layer.duration_nanos,
            effect: CompiledEffect::Brightness {
                amount: CompiledScalarProperty {
                    authored_track: CompiledTrack::new(0.1),
                    modifiers: vec![CompiledScalarModifier {
                        operation: ScalarModifierOperation::Add,
                        signal: ScalarSignalId::new(0),
                    }],
                    constraint: ScalarPropertyConstraint::Finite,
                },
            },
            dependency: TemporalDependency::Dynamic,
        });
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(5_000_000_000, 500_000_000, vec![0.0, 1.0]).expect("signal"),
        ]);
        let frame = evaluate_with_context(
            &plan,
            &[super::ScheduledItem(0)],
            5_500_000_000,
            &EvaluationContext::new(&signals),
        )
        .expect("evaluation");
        // At project time 5.5s the authored track uses clip-local time 0.5s
        // (0.8), while the signal uses absolute project time 5.5s (1.0).
        // Generated opacity then multiplies the modulated value before the
        // final clamp: (0.8 + 1.0) * 0.5 = 0.9.
        assert!(
            (frame.layers[0].opacity - 0.9).abs() < 1.0e-12,
            "opacity was {}",
            frame.layers[0].opacity
        );
        assert!(matches!(
            frame.layers[0].effects.last(),
            Some(super::EvaluatedEffect::Brightness { amount }) if (*amount - 1.1).abs() < 1.0e-12
        ));
    }

    #[test]
    fn prepared_envelope_signal_replaces_brightness_at_absolute_project_time() {
        let mut plan = compile(canonical_input(), CompileOptions::default()).expect("plan");
        let layer = &mut plan.layers[0];
        stage_layer(layer, 5.0, 9.0);
        layer.effects.push(TimedEffect {
            start: 0,
            end: layer.duration_nanos,
            effect: CompiledEffect::Brightness {
                amount: CompiledScalarProperty {
                    authored_track: CompiledTrack {
                        base_value: 0.0,
                        keyframes: vec![CompiledKeyframe {
                            time: 1_000_000_000,
                            value: 0.25,
                            interpolation: CompiledInterpolation::Linear,
                        }],
                    },
                    modifiers: vec![CompiledScalarModifier {
                        operation: ScalarModifierOperation::Replace,
                        signal: ScalarSignalId::new(0),
                    }],
                    constraint: ScalarPropertyConstraint::Finite,
                },
            },
            dependency: TemporalDependency::Dynamic,
        });
        let source = RawScalarSignal::Audio(super::AudioScalarSignal {
            tap: super::AudioAnalysisTap::Master,
            feature: super::AudioScalarFeature::Rms,
        });
        let compiled = CompiledScalarSignals::from_signals(vec![CompiledScalarSignal::new(
            source,
            vec![
                CompiledSignalTransform::Gain(super::GainTransform::new(1.0).unwrap()),
                CompiledSignalTransform::Envelope(EnvelopeTransform::new(
                    1_000_000_000,
                    1_000_000_000,
                )),
            ],
        )]);
        let signals = prepare_scalar_signals(
            &compiled,
            BTreeMap::from([(
                super::AudioAnalysisRequirement::Master(super::AudioScalarFeature::Rms),
                PreparedScalarSignal::new(5_000_000_000, 1_000_000_000, vec![0.0, 1.0, 1.0, 1.0])
                    .unwrap(),
            )]),
        )
        .unwrap();
        let context = EvaluationContext::new(&signals);
        for (project_time, expected) in [
            (8_000_000_000, 1.0 - (-3.0_f64).exp()),
            (6_000_000_000, 1.0 - (-1.0_f64).exp()),
            (7_000_000_000, 1.0 - (-2.0_f64).exp()),
        ] {
            let frame =
                evaluate_with_context(&plan, &[super::ScheduledItem(0)], project_time, &context)
                    .unwrap();
            assert!(matches!(
                frame.layers[0].effects.last(),
                Some(super::EvaluatedEffect::Brightness { amount }) if (*amount - expected).abs() < 1.0e-12
            ));
        }
    }

    #[test]
    fn motion_blur_samples_transform_modifiers_at_each_historical_project_time() {
        let mut plan = compile(canonical_input(), CompileOptions::default()).expect("plan");
        let index = plan
            .layers
            .iter()
            .position(|layer| matches!(layer.source, super::CompiledVisualSource::Image { .. }))
            .expect("image layer");
        let canvas_width = f64::from(plan.canvas.width);
        let frame_duration_nanos =
            (1_000_000_000_u128 * u128::from(plan.frame_rate.1)) / u128::from(plan.frame_rate.0);
        let expected_radius = canvas_width * frame_duration_nanos as f64 / 1_000_000_000.0;
        let layer = &mut plan.layers[index];
        stage_layer(layer, 5.0, 7.0);
        layer.transform.position = CompiledTrack::new(crate::domain::Point { x: 0.0, y: 0.0 });
        layer.transform_contributions.clear();
        layer.transform.position_x_modifiers = vec![CompiledScalarModifier {
            operation: ScalarModifierOperation::Add,
            signal: ScalarSignalId::new(0),
        }];
        layer.effects.push(TimedEffect {
            start: 0,
            end: layer.duration_nanos,
            effect: CompiledEffect::MotionBlur {
                intensity: CompiledScalarProperty::authored(CompiledTrack::new(1.0)),
                shutter_angle: CompiledScalarProperty::authored(CompiledTrack::new(360.0)),
                max_radius: CompiledScalarProperty::authored(CompiledTrack::new(32.0)),
                samples: 8,
            },
            dependency: TemporalDependency::Dynamic,
        });
        let signals = PreparedScalarSignals::new(vec![
            PreparedScalarSignal::new(5_000_000_000, 1_000_000_000, vec![0.0, 1.0, 2.0])
                .expect("signal"),
        ]);
        let frame = evaluate_with_context(
            &plan,
            &[super::ScheduledItem(index)],
            6_000_000_000,
            &EvaluationContext::new(&signals),
        )
        .expect("evaluation");
        assert!(matches!(
            frame.layers[0].effects.last(),
            Some(super::EvaluatedEffect::MotionBlur { radius, angle_degrees, .. })
                if (*radius - expected_radius).abs() < 1.0e-9
                    && angle_degrees.abs() < 1.0e-12
        ));
    }

    #[test]
    fn compiler_normalizes_tracks_effects_fusion_and_dependencies() {
        let mut project = canonical_project();
        let blue = project
            .visual
            .clips
            .iter_mut()
            .find(|clip| clip.id == "blue-in")
            .expect("static clip");
        blue.opacity = Track {
            base_value: 1.0,
            keyframes: vec![
                Keyframe {
                    time: 0.0,
                    value: 1.0,
                    interpolation: Interpolation::Named(InterpolationName::Linear),
                },
                Keyframe {
                    time: 1.0,
                    value: 1.0,
                    interpolation: Interpolation::Named(InterpolationName::EaseInOut),
                },
            ],
        }
        .into();
        blue.effects = vec![
            Effect::Brightness {
                id: "brightness".into(),
                amount: Track::constant(0.1).into(),
            },
            Effect::Contrast {
                id: "contrast".into(),
                amount: Track::constant(1.1).into(),
            },
            Effect::Saturation {
                id: "saturation".into(),
                amount: Track::constant(0.9).into(),
            },
            Effect::CameraShake {
                id: "zero-shake".into(),
                timing: ActiveInterval::default(),
                position_amount: Track::constant(0.0).into(),
                rotation_degrees: Track::constant(0.0).into(),
                scale_amount: Track::constant(0.0).into(),
                frequency: Track::constant(5.0).into(),
                seed: 7,
                attack: 0.0,
                decay: 1.0,
            },
        ];
        let mut third_static = blue.clone();
        third_static.id = "blue-static-copy".into();
        third_static.start = 0.0;
        project.visual.clips.push(third_static);
        project.visual.transitions.clear();
        project.visual.flashes.clear();
        let plan = compile_project(project);
        let blue = plan
            .layers
            .iter()
            .find(|layer| layer.id == "blue-in")
            .expect("compiled static clip");
        assert!(blue.opacity.authored_track.keyframes.is_empty());
        assert_eq!(blue.content_dependency, TemporalDependency::Static);
        assert!(matches!(
            blue.effects.as_slice(),
            [super::TimedEffect {
                effect: CompiledEffect::ColourTransform { .. },
                dependency: TemporalDependency::Static,
                ..
            }]
        ));
        assert_eq!(plan.compilation.static_layer_count, 2);
        assert_eq!(plan.compilation.dynamic_layer_count, 1);
        assert_eq!(plan.compilation.constant_track_normalization_count, 2);
        assert_eq!(plan.compilation.effect_count_before_normalization, 9);
        assert_eq!(plan.compilation.effect_count_after_normalization, 3);
    }

    #[test]
    fn authored_and_generated_gaussian_blur_share_the_target_constraint() {
        let mut project = canonical_project();
        let clip = &mut project.visual.clips[0];
        clip.effects = vec![Effect::GaussianBlur {
            id: "authored-blur".into(),
            radius: Track::constant(2.0).into(),
        }];
        clip.preset = Some(Preset::FocusReveal {
            timing: ActiveInterval {
                start: 0.0,
                duration: Some(0.5),
            },
            intensity: 1.0,
        });

        let plan = compile_project(project);
        let layer = &plan.layers[0];
        let constraints = layer
            .effects
            .iter()
            .filter_map(|timed| match &timed.effect {
                CompiledEffect::GaussianBlur { radius } => Some(radius.constraint),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(constraints.len(), 2);
        assert!(constraints.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(
            constraints[0],
            ScalarPropertyConstraint::ClosedRange {
                min: 0.0,
                max: 32.0,
            }
        );
    }

    #[test]
    fn compiler_removes_exact_identities_but_keeps_near_values_and_classifies_post_effects() {
        let mut project = canonical_project();
        project.visual.post_effects = vec![
            Effect::Brightness {
                id: "identity".into(),
                amount: Track::constant(0.0).into(),
            },
            Effect::Contrast {
                id: "near".into(),
                amount: Track::constant(1.000_001).into(),
            },
        ];
        let plan = compile_project(project.clone());
        assert_eq!(plan.post_effects.len(), 1);
        assert_eq!(plan.post_effect_dependency, TemporalDependency::Static);

        project.visual.post_effects[1] = Effect::Contrast {
            id: "animated".into(),
            amount: Track {
                base_value: 1.0,
                keyframes: vec![Keyframe {
                    time: 1.0,
                    value: 1.1,
                    interpolation: Interpolation::Named(InterpolationName::Linear),
                }],
            }
            .into(),
        };
        let plan = compile_project(project);
        assert_eq!(plan.post_effect_dependency, TemporalDependency::Dynamic);
    }

    #[test]
    fn compiler_separates_static_clip_content_from_timeline_and_transition_animation() {
        let mut project = canonical_project();
        project.visual.transitions.clear();
        let plan = compile_project(project.clone());
        let blue = plan
            .layers
            .iter()
            .find(|layer| layer.id == "blue-in")
            .expect("compiled static clip");
        assert_eq!(blue.content_dependency, TemporalDependency::Static);

        project
            .visual
            .clips
            .iter_mut()
            .find(|clip| clip.id == "blue-in")
            .expect("static clip")
            .transform
            .as_mut()
            .expect("image transform")
            .scale
            .keyframes = vec![Keyframe {
            time: 1.0,
            value: crate::project::Point { x: 1.1, y: 1.1 },
            interpolation: Interpolation::Named(InterpolationName::Linear),
        }];
        let plan = compile_project(project.clone());
        assert_eq!(
            plan.layers
                .iter()
                .find(|layer| layer.id == "blue-in")
                .expect("compiled animated transform")
                .content_dependency,
            TemporalDependency::Dynamic
        );

        let blue = project
            .visual
            .clips
            .iter_mut()
            .find(|clip| clip.id == "blue-in")
            .expect("static clip");
        blue.transform
            .as_mut()
            .expect("image transform")
            .scale
            .keyframes
            .clear();
        blue.opacity.track.keyframes = vec![Keyframe {
            time: 1.0,
            value: 0.5,
            interpolation: Interpolation::Named(InterpolationName::Linear),
        }];
        let plan = compile_project(project);
        assert_eq!(
            plan.layers
                .iter()
                .find(|layer| layer.id == "blue-in")
                .expect("compiled animated opacity")
                .content_dependency,
            TemporalDependency::Dynamic
        );
    }

    #[test]
    fn compiler_classifies_whole_visual_activity_conservatively() {
        let mut project = canonical_project();
        project.visual.transitions.clear();
        assert_eq!(
            compile_project(project.clone()).visual_dependency,
            TemporalDependency::Dynamic
        );

        let clip = project.visual.clips.remove(1);
        project.visual.clips = vec![clip];
        project.visual.clips[0].start = 0.0;
        project.visual.clips[0].duration = 6.0;
        project.visual.flashes.clear();
        project.visual.post_effects.clear();
        assert_eq!(
            compile_project(project.clone()).visual_dependency,
            TemporalDependency::Static
        );

        project.visual.clips.clear();
        assert_eq!(
            compile_project(project).visual_dependency,
            TemporalDependency::Static
        );
    }

    #[test]
    fn compiler_normalization_is_deterministic() {
        let mut project = canonical_project();
        let blue = project
            .visual
            .clips
            .iter_mut()
            .find(|clip| clip.id == "blue-in")
            .expect("static clip");
        blue.effects = vec![
            Effect::Brightness {
                id: "brightness".into(),
                amount: Track::constant(0.1).into(),
            },
            Effect::Contrast {
                id: "contrast".into(),
                amount: Track::constant(1.1).into(),
            },
        ];
        let first = compile_project(project.clone());
        let second = compile_project(project);
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }

    #[test]
    fn disabled_output_audio_preserves_the_authored_master_mix() {
        let mut project: Project = serde_json::from_str(include_str!(
            "../../../../examples/projects/audio-static-mix.json"
        ))
        .expect("fixture project");
        project.output.audio = false;
        let project = Box::leak(Box::new(project));
        let assets = Box::leak(Box::new(BTreeMap::from([(
            "tone".to_owned(),
            PathBuf::from("/resolved/tone.wav"),
        )])));
        let durations = Box::leak(Box::new(BTreeMap::from([("tone".to_owned(), 2.0)])));
        let plan = compile(
            PlanCompileInput::new(
                project,
                ResourceLimits::default(),
                std::path::Path::new("/projects"),
                assets,
                durations,
                2.0,
                (30, 1),
                60,
                &[],
            ),
            CompileOptions::default(),
        )
        .expect("plan");
        assert!(!plan.audio_output_enabled);
        assert!(plan.encoder.audio_mix.is_none());
        assert!(plan.audio_mix.has_authored_material());
    }
}
