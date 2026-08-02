//! Deterministic compilation and per-frame evaluation for renderer backends.

mod compiler;
mod effect_passes;
mod evaluation;
mod input;
mod model;
mod schedule;

pub use compiler::{CompileOptions, compile};
pub use effect_passes::{EffectPass, EffectPassPlan, effect_pass_plan};
pub use evaluation::evaluate;
pub use evaluation::{EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource};
pub use input::PlanCompileInput;
pub use model::*;
pub use schedule::{
    ActiveSchedule, ScheduleAction, ScheduleCursor, ScheduleEvent, sort_active_items,
};

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::{ActiveSchedule, CompileOptions, PlanCompileInput, compile, evaluate};
    use crate::{
        plan::{CompiledEffect, TemporalDependency},
        project::{
            ActiveInterval, Effect, Interpolation, InterpolationName, Keyframe, Project, Track,
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
        let frame = evaluate(&plan, &active, 0);
        assert_eq!(frame.layers.len(), 1);
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
        };
        blue.effects = vec![
            Effect::Brightness {
                id: "brightness".into(),
                amount: Track::constant(0.1),
            },
            Effect::Contrast {
                id: "contrast".into(),
                amount: Track::constant(1.1),
            },
            Effect::Saturation {
                id: "saturation".into(),
                amount: Track::constant(0.9),
            },
            Effect::CameraShake {
                id: "zero-shake".into(),
                timing: ActiveInterval::default(),
                position_amount: Track::constant(0.0),
                rotation_degrees: Track::constant(0.0),
                scale_amount: Track::constant(0.0),
                frequency: Track::constant(5.0),
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
        assert!(blue.opacity.keyframes.is_empty());
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
    fn compiler_removes_exact_identities_but_keeps_near_values_and_classifies_post_effects() {
        let mut project = canonical_project();
        project.visual.post_effects = vec![
            Effect::Brightness {
                id: "identity".into(),
                amount: Track::constant(0.0),
            },
            Effect::Contrast {
                id: "near".into(),
                amount: Track::constant(1.000_001),
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
            },
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
        blue.opacity.keyframes = vec![Keyframe {
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
                amount: Track::constant(0.1),
            },
            Effect::Contrast {
                id: "contrast".into(),
                amount: Track::constant(1.1),
            },
        ];
        let first = compile_project(project.clone());
        let second = compile_project(project);
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }
}
