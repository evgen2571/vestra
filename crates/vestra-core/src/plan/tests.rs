use std::{collections::BTreeMap, path::PathBuf};

use super::{
    ActiveSchedule, CompileOptions, CompiledScalarModifier, CompiledScalarProperty,
    CompiledScalarSignal, CompiledScalarSignals, CompiledSignalTransform, EnvelopeTransform,
    EvaluationContext, PlanCompileInput, PreparedScalarSignal, PreparedScalarSignals,
    RawScalarSignal, ScalarModifierOperation, ScalarPropertyConstraint, ScalarSignalId,
    TimedEffect, compile, evaluate, evaluate_with_context, prepare_scalar_signals,
};
use crate::{
    animation::{
        Interpolation as CompiledInterpolation, Keyframe as CompiledKeyframe,
        Track as CompiledTrack,
    },
    plan::{CompiledEffect, TemporalDependency},
    project::{
        ActiveInterval, AudioAnalysisTap as ProjectAudioAnalysisTap,
        AudioScalarFeature as ProjectAudioScalarFeature, Effect, Group, Interpolation,
        InterpolationName, Keyframe, NormalizedKeyframe, NormalizedTrack, ParticleBurst,
        ParticleEmission, ParticleSystem, Preset, Project, ScalarModifier,
        ScalarModifierOperation as ProjectScalarModifierOperation, ScalarSignal,
        ScalarSignalSource, SignalTransform, Spectrum2D, Spectrum2DBandMapping, Spectrum2DLayout,
        Spectrum2DLinearAnchor, Spectrum2DLinearLayout, Track, TransitionDefinition,
        TransitionPlacement, TransitionPresentation, VisualSource,
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
    compile_project_with_limits(project, ResourceLimits::default())
}

fn compile_project_with_limits(project: Project, limits: ResourceLimits) -> super::RenderPlan {
    compile_project_result(project, limits).expect("plan")
}

fn compile_project_result(
    project: Project,
    limits: ResourceLimits,
) -> Result<super::RenderPlan, Box<crate::Diagnostic>> {
    let project = Box::leak(Box::new(project));
    let assets = Box::leak(Box::new(BTreeMap::from([
        ("red".to_owned(), PathBuf::from("/resolved/red.png")),
        ("blue".to_owned(), PathBuf::from("/resolved/blue.png")),
    ])));
    let durations = Box::leak(Box::new(BTreeMap::new()));
    compile(
        PlanCompileInput::new(
            project,
            limits,
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
    .map_err(Box::new)
}

fn canonical_project() -> Project {
    serde_json::from_str(include_str!(
        "../../../../examples/projects/animation-effects.json"
    ))
    .expect("fixture project")
}

#[test]
fn groups_compile_nested_layers_and_use_local_time_with_parent_clipping() {
    let mut project = canonical_project();
    let mut child = project.visual.clips[0].clone();
    child.id = "nested-child".to_owned();
    child.start = 2.0;
    child.duration = 10.0;
    let mut parent = child.clone();
    parent.id = "group".to_owned();
    parent.start = 10.0;
    parent.duration = 5.0;
    parent.source = VisualSource::Group(Group { clips: vec![child] });
    parent.transform = None;
    project.visual.clips = vec![parent];
    project.visual.transitions.clear();

    let plan = compile_project(project);
    let super::CompiledVisualSource::Group(composition) = &plan.layers[0].source else {
        panic!("expected compiled Group source");
    };
    assert_eq!(composition.layers.len(), 1);
    assert_eq!(composition.layers[0].start_nanos, 2_000_000_000);
    assert!(
        composition
            .schedule
            .active_at_time(&composition.layers, 1_999_999_999)
            .is_empty()
    );
    assert_eq!(
        composition
            .schedule
            .active_at_time(&composition.layers, 2_000_000_000)
            .len(),
        1
    );

    let before = evaluate(&plan, &[super::ScheduledItem(0)], 11_999_999_999).expect("frame");
    let at_start = evaluate(&plan, &[super::ScheduledItem(0)], 12_000_000_000).expect("frame");
    let super::EvaluatedSource::Group {
        composition: before_group,
        ..
    } = &before.layers[0].source
    else {
        panic!("expected evaluated Group source");
    };
    assert!(before_group.layers.is_empty());
    let super::EvaluatedSource::Group {
        composition: start_group,
        ..
    } = &at_start.layers[0].source
    else {
        panic!("expected evaluated Group source");
    };
    assert_eq!(start_group.layers.len(), 1);
}

#[test]
fn static_group_dependency_ignores_children_outside_its_effective_interval() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let mut child = project.visual.clips[1].clone();
    child.id = "group-child".to_owned();
    child.start = 0.0;
    child.duration = 6.0;
    child.effects = vec![Effect::Brightness {
        id: "nested-brightness".to_owned(),
        amount: Track::constant(0.1).into(),
    }];
    let mut group = child.clone();
    group.id = "static-group".to_owned();
    group.source = VisualSource::Group(Group {
        clips: vec![child.clone()],
    });
    group.transform = None;
    group.effects.clear();
    group.start = 0.0;
    group.duration = 6.0;
    project.visual.clips = vec![group];

    let plan = compile_project(project.clone());
    let super::CompiledVisualSource::Group(composition) = &plan.layers[0].source else {
        panic!("expected Group source");
    };
    assert_eq!(composition.dependency, TemporalDependency::Static);
    assert_eq!(
        plan.layers[0].content_dependency,
        TemporalDependency::Static
    );
    assert_eq!(plan.compilation.image_source_count, 1);
    assert_eq!(plan.compilation.static_layer_count, 2);
    assert_eq!(plan.compilation.local_effect_count, 1);

    let mut outside = child;
    outside.id = "outside".to_owned();
    outside.start = 10.0;
    outside.duration = 10.0;
    if let VisualSource::Group(group) = &mut project.visual.clips[0].source {
        group.clips = vec![outside];
    }
    let plan = compile_project(project);
    let super::CompiledVisualSource::Group(composition) = &plan.layers[0].source else {
        panic!("expected Group source");
    };
    assert_eq!(composition.dependency, TemporalDependency::Static);
}

#[test]
fn group_dependency_tracks_membership_and_nested_dynamic_content() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let mut first = project.visual.clips[0].clone();
    first.id = "first".to_owned();
    first.start = 0.0;
    first.duration = 3.0;
    let mut second = first.clone();
    second.id = "second".to_owned();
    second.start = 3.0;
    second.duration = 3.0;
    let mut group = first.clone();
    group.id = "group".to_owned();
    group.source = VisualSource::Group(Group {
        clips: vec![first.clone(), second],
    });
    group.transform = None;
    group.start = 0.0;
    group.duration = 6.0;
    project.visual.clips = vec![group];
    let plan = compile_project(project.clone());
    let super::CompiledVisualSource::Group(composition) = &plan.layers[0].source else {
        panic!("expected Group source");
    };
    assert_eq!(composition.dependency, TemporalDependency::Dynamic);

    let mut animated = first;
    animated.id = "animated".to_owned();
    animated.opacity.track.keyframes = vec![Keyframe {
        time: 1.0,
        value: 0.5,
        interpolation: Interpolation::Named(InterpolationName::Linear),
    }];
    let mut inner = animated.clone();
    inner.id = "inner".to_owned();
    inner.source = VisualSource::Group(Group {
        clips: vec![animated],
    });
    inner.transform = None;
    inner.start = 0.0;
    inner.duration = 6.0;
    project.visual.clips[0].source = VisualSource::Group(Group { clips: vec![inner] });
    let plan = compile_project(project);
    assert_eq!(
        plan.layers[0].content_dependency,
        TemporalDependency::Dynamic
    );
}

#[test]
fn effective_ancestor_window_excludes_invisible_nested_layers_from_limits() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let mut child = project.visual.clips[0].clone();
    child.start = 5.0;
    child.duration = 5.0;
    child.id = "a".to_owned();
    let mut second = child.clone();
    second.id = "b".to_owned();
    let mut group = child.clone();
    group.id = "group".to_owned();
    group.start = 5.0;
    group.duration = 20.0;
    group.transform = None;
    group.source = VisualSource::Group(Group {
        clips: vec![child, second],
    });
    project.visual.clips = vec![group];
    let limits = ResourceLimits {
        maximum_active_layers: 1,
        ..ResourceLimits::default()
    };
    compile_project_with_limits(project, limits);
}

#[test]
fn root_transitions_accept_groups_without_resolving_nested_ids() {
    let mut project = canonical_project();
    let child = project.visual.clips[0].clone();
    let mut group = child.clone();
    group.id = "group".to_owned();
    group.source = VisualSource::Group(Group { clips: vec![child] });
    group.transform = None;
    group.start = 0.0;
    group.duration = 6.0;
    let mut image = project.visual.clips[1].clone();
    image.id = "root-image".to_owned();
    image.start = 0.0;
    image.duration = 6.0;
    project.visual.clips = vec![group, image];
    project.visual.transitions = vec![TransitionPlacement {
        id: "group-crossfade".to_owned(),
        outgoing: "group".to_owned(),
        incoming: "root-image".to_owned(),
        start: 1.0,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(NormalizedTrack {
                    keyframes: vec![
                        NormalizedKeyframe {
                            progress: 0.0,
                            value: 1.0,
                            interpolation: Interpolation::Named(InterpolationName::Linear),
                        },
                        NormalizedKeyframe {
                            progress: 1.0,
                            value: 0.0,
                            interpolation: Interpolation::Named(InterpolationName::Linear),
                        },
                    ],
                }),
                ..Default::default()
            },
            incoming: TransitionPresentation {
                opacity: Some(NormalizedTrack {
                    keyframes: vec![
                        NormalizedKeyframe {
                            progress: 0.0,
                            value: 0.0,
                            interpolation: Interpolation::Named(InterpolationName::Linear),
                        },
                        NormalizedKeyframe {
                            progress: 1.0,
                            value: 1.0,
                            interpolation: Interpolation::Named(InterpolationName::Linear),
                        },
                    ],
                }),
                ..Default::default()
            },
        },
    }];

    let plan = compile_project(project);
    assert!(plan.layers[0].opacity_contributions.len() == 1);
    assert_eq!(
        plan.layers[0].content_dependency,
        TemporalDependency::Dynamic
    );
}

#[test]
fn nested_clips_use_root_preset_and_normalization_semantics_recursively() {
    let mut root_project = canonical_project();
    root_project.visual.transitions.clear();
    let mut root_clip = root_project.visual.clips[0].clone();
    root_clip.preset = Some(Preset::FocusReveal {
        timing: ActiveInterval {
            start: 0.0,
            duration: Some(0.5),
        },
        intensity: 1.0,
    });
    root_project.visual.clips = vec![root_clip.clone()];
    let root = compile_project(root_project);

    let mut nested_project = canonical_project();
    nested_project.visual.transitions.clear();
    let mut group = root_clip.clone();
    group.id = "group".into();
    group.preset = None;
    group.transform = None;
    group.source = VisualSource::Group(Group {
        clips: vec![root_clip.clone()],
    });
    nested_project.visual.clips = vec![group];
    let nested = compile_project(nested_project);

    let super::CompiledVisualSource::Group(composition) = &nested.layers[0].source else {
        panic!("expected Group");
    };
    let child = &composition.layers[0];
    assert_eq!(child.effects.len(), root.layers[0].effects.len());
    assert_eq!(
        child.transform_contributions.len(),
        root.layers[0].transform_contributions.len()
    );
    assert_eq!(child.content_dependency, root.layers[0].content_dependency);
    assert!(child.opacity.authored_track.keyframes.is_empty());

    let mut deep_project = canonical_project();
    deep_project.visual.transitions.clear();
    let mut inner = group_clip_with_child(root_clip);
    inner.id = "inner".into();
    let mut outer = inner.clone();
    outer.id = "outer".into();
    outer.source = VisualSource::Group(Group { clips: vec![inner] });
    deep_project.visual.clips = vec![outer];
    let deep = compile_project(deep_project);
    let super::CompiledVisualSource::Group(outer_composition) = &deep.layers[0].source else {
        panic!("expected outer Group");
    };
    let super::CompiledVisualSource::Group(inner_composition) = &outer_composition.layers[0].source
    else {
        panic!("expected inner Group");
    };
    assert_eq!(
        inner_composition.layers[0].effects.len(),
        root.layers[0].effects.len()
    );
}

fn group_clip_with_child(child: crate::project::Clip) -> crate::project::Clip {
    let mut group = child.clone();
    group.source = VisualSource::Group(Group { clips: vec![child] });
    group.preset = None;
    group.transform = None;
    group
}

#[test]
fn nested_compositions_enforce_active_layer_limits_independently() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let template = project.visual.clips[0].clone();
    let children = (0..3)
        .map(|index| {
            let mut child = template.clone();
            child.id = format!("child-{index}");
            child.start = 0.0;
            child.duration = 2.0;
            child
        })
        .collect();
    let mut group = template;
    group.id = "group".into();
    group.start = 0.0;
    group.duration = 2.0;
    group.transform = None;
    group.source = VisualSource::Group(Group { clips: children });
    project.visual.clips = vec![group];

    let limits = ResourceLimits {
        maximum_active_layers: 2,
        ..ResourceLimits::default()
    };
    let project = Box::leak(Box::new(project));
    let assets = Box::leak(Box::new(BTreeMap::from([
        ("red".to_owned(), PathBuf::from("/resolved/red.png")),
        ("blue".to_owned(), PathBuf::from("/resolved/blue.png")),
    ])));
    let durations = Box::leak(Box::new(BTreeMap::new()));
    let error = compile(
        PlanCompileInput::new(
            project,
            limits,
            std::path::Path::new("/projects"),
            assets,
            durations,
            2.0,
            (24, 1),
            48,
            &[],
        ),
        CompileOptions::default(),
    )
    .expect_err("nested active-layer limit must not be bypassed");
    assert_eq!(error.code, "MVP-LIMIT-ACTIVE-LAYERS");
}

#[test]
fn nested_active_layer_limit_ignores_children_after_group_duration() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    project.visual.flashes.clear();
    let template = project.visual.clips[0].clone();
    let children = (0..3)
        .map(|index| {
            let mut child = template.clone();
            child.id = format!("child-{index}");
            child.start = 10.0;
            child.duration = 10.0;
            child
        })
        .collect();
    let mut group = template;
    group.id = "group".into();
    group.start = 0.0;
    group.duration = 5.0;
    group.transform = None;
    group.source = VisualSource::Group(Group { clips: children });
    project.visual.clips = vec![group];

    let plan = compile_project_with_limits(
        project,
        ResourceLimits {
            maximum_active_layers: 2,
            ..ResourceLimits::default()
        },
    );
    let super::CompiledVisualSource::Group(composition) = &plan.layers[0].source else {
        panic!("expected Group");
    };
    assert_eq!(composition.layers[0].start_nanos, 10_000_000_000);
}

#[test]
fn nested_active_layer_limit_counts_only_partial_overlap_before_group_end() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    project.visual.flashes.clear();
    let template = project.visual.clips[0].clone();
    let children = [("a", 0.0), ("b", 4.0), ("c", 6.0)]
        .into_iter()
        .map(|(id, start)| {
            let mut child = template.clone();
            child.id = id.into();
            child.start = start;
            child.duration = 10.0;
            child
        })
        .collect();
    let mut group = template;
    group.id = "group".into();
    group.start = 0.0;
    group.duration = 5.0;
    group.transform = None;
    group.source = VisualSource::Group(Group { clips: children });
    project.visual.clips = vec![group];

    let error = compile_project_result(
        project.clone(),
        ResourceLimits {
            maximum_active_layers: 1,
            ..ResourceLimits::default()
        },
    )
    .expect_err("peak overlap must exceed limit 1");
    assert_eq!(error.code, "MVP-LIMIT-ACTIVE-LAYERS");
    compile_project_with_limits(
        project,
        ResourceLimits {
            maximum_active_layers: 2,
            ..ResourceLimits::default()
        },
    );
}

#[test]
fn child_starting_at_group_end_is_not_active() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    project.visual.flashes.clear();
    let template = project.visual.clips[0].clone();
    let mut child = template.clone();
    child.id = "child".into();
    child.start = 5.0;
    child.duration = 10.0;
    let mut group = template;
    group.id = "group".into();
    group.start = 0.0;
    group.duration = 5.0;
    group.transform = None;
    group.source = VisualSource::Group(Group { clips: vec![child] });
    project.visual.clips = vec![group];

    compile_project_with_limits(
        project,
        ResourceLimits {
            maximum_active_layers: 1,
            ..ResourceLimits::default()
        },
    );
}

#[test]
fn deeply_nested_active_layer_limit_uses_inner_group_duration() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    project.visual.flashes.clear();
    let template = project.visual.clips[0].clone();
    let children = (0..3)
        .map(|index| {
            let mut child = template.clone();
            child.id = format!("child-{index}");
            child.start = 6.0;
            child.duration = 4.0;
            child
        })
        .collect();
    let mut inner = template.clone();
    inner.id = "inner".into();
    inner.start = 0.0;
    inner.duration = 5.0;
    inner.transform = None;
    inner.source = VisualSource::Group(Group { clips: children });
    let mut outer = template;
    outer.id = "outer".into();
    outer.start = 0.0;
    outer.duration = 10.0;
    outer.transform = None;
    outer.source = VisualSource::Group(Group { clips: vec![inner] });
    project.visual.clips = vec![outer];

    compile_project_with_limits(
        project,
        ResourceLimits {
            maximum_active_layers: 1,
            ..ResourceLimits::default()
        },
    );
}

fn camera_shake(position_amount: f64) -> Effect {
    Effect::CameraShake {
        id: "shake".into(),
        timing: ActiveInterval::default(),
        position_amount: Track::constant(position_amount).into(),
        rotation_degrees: Track::constant(0.2).into(),
        scale_amount: Track::constant(0.05).into(),
        frequency: Track::constant(14.0).into(),
        seed: 7,
        attack: 0.0,
        decay: 1.0,
    }
}

#[test]
fn camera_shake_targets_group_transform_and_preserves_child_transform() {
    let mut without = canonical_project();
    without.visual.transitions.clear();
    let child_template = without.visual.clips[0].clone();
    let mut child = child_template.clone();
    child.id = "child".into();
    child.effects.clear();
    let mut group = child_template;
    group.id = "group".into();
    group.start = 0.0;
    group.duration = 2.0;
    group.transform = None;
    group.effects.clear();
    group.source = VisualSource::Group(Group { clips: vec![child] });
    without.visual.clips = vec![group];

    let mut with_shake = without.clone();
    with_shake.visual.clips[0].effects = vec![camera_shake(0.1)];
    let plain = compile_project(without);
    let shaken = compile_project(with_shake);
    let plain_frame = evaluate(&plain, &[super::ScheduledItem(0)], 500_000_000).expect("frame");
    let shaken_frame = evaluate(&shaken, &[super::ScheduledItem(0)], 500_000_000).expect("frame");
    let super::EvaluatedSource::Group {
        composition: plain_composition,
        transform: plain_transform,
    } = &plain_frame.layers[0].source
    else {
        panic!("expected plain Group");
    };
    let super::EvaluatedSource::Group {
        composition: shaken_composition,
        transform: shaken_transform,
    } = &shaken_frame.layers[0].source
    else {
        panic!("expected shaken Group");
    };
    assert_ne!(plain_transform.position, shaken_transform.position);
    let super::EvaluatedSource::Image {
        transform: plain_child_transform,
        ..
    } = &plain_composition.layers[0].source
    else {
        panic!("expected plain Image child");
    };
    let super::EvaluatedSource::Image {
        transform: shaken_child_transform,
        ..
    } = &shaken_composition.layers[0].source
    else {
        panic!("expected shaken Image child");
    };
    assert_eq!(plain_child_transform, shaken_child_transform);
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
    layer.end_frame = crate::plan_time::first_frame_at_or_after(end, (24, 1)).expect("end frame");
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
fn spectrum2d_compiles_logarithmic_bands_into_interned_audio_signals() {
    let mut project = canonical_project();
    let spectrum = Spectrum2D {
        band_count: 24,
        min_hz: 40.0,
        max_hz: 16_000.0,
        sensitivity: 3.5,
        attack_seconds: 0.03,
        release_seconds: 0.2,
        layout: Spectrum2DLayout::Linear(Spectrum2DLinearLayout {
            anchor: Spectrum2DLinearAnchor::Bottom,
            band_mapping: Spectrum2DBandMapping::CenterOut,
        }),
        ..Spectrum2D::default()
    };
    project.visual.clips[0].source = VisualSource::Spectrum2D(spectrum.clone());
    project.visual.clips[0].transform = None;
    project.visual.clips[1].source = VisualSource::Spectrum2D(Spectrum2D {
        x: 0.0,
        ..spectrum.clone()
    });
    project.visual.clips[1].transform = None;
    let plan = compile_project(project);

    assert_eq!(plan.scalar_signals.len(), 24);
    assert_eq!(plan.audio_analysis_requirements.iter().len(), 24);
    assert_eq!(plan.compilation.spectrum2d_source_count, 2);
    assert!(plan.compilation.dynamic_layer_count >= 2);
    let generated_bands = spectrum.logarithmic_bands();
    let first_layer = plan
        .layers
        .iter()
        .find_map(|layer| match &layer.source {
            super::CompiledVisualSource::Spectrum2D { band_signals, .. } => Some(band_signals),
            _ => None,
        })
        .expect("Spectrum2D layer");
    assert_eq!(first_layer.len(), generated_bands.len());
    for (signal_id, (expected_min, expected_max)) in first_layer.iter().zip(generated_bands) {
        let signal = plan
            .scalar_signals
            .get(*signal_id)
            .expect("compiled signal");
        let RawScalarSignal::Audio(audio) = signal.source;
        let super::AudioScalarFeature::BandEnergy(band) = audio.feature else {
            panic!("expected BandEnergy signal");
        };
        assert!((band.min_hz() - expected_min).abs() < 1.0e-9);
        assert!((band.max_hz() - expected_max).abs() < 1.0e-9);
        assert!(
            matches!(signal.transforms[0], CompiledSignalTransform::Gain(gain) if (gain.gain() - spectrum.sensitivity).abs() < 1.0e-12)
        );
        assert!(
            matches!(signal.transforms[1], CompiledSignalTransform::Clamp(clamp) if clamp.min() == 0.0 && clamp.max() == 1.0)
        );
        assert!(
            matches!(signal.transforms[2], CompiledSignalTransform::Envelope(envelope) if envelope.attack() == 30_000_000 && envelope.release() == 200_000_000)
        );
    }
    assert!(
        plan.layers
            .iter()
            .filter(|layer| matches!(
                layer.source,
                super::CompiledVisualSource::Spectrum2D { ref band_signals, .. }
                    if band_signals.len() == 24
            ))
            .all(|layer| layer.content_dependency == TemporalDependency::Dynamic)
    );
}

#[test]
fn spectrum2d_evaluation_samples_absolute_project_time_deterministically() {
    let mut project = canonical_project();
    project.visual.clips[0].source = VisualSource::Spectrum2D(Spectrum2D::default());
    project.visual.clips[0].transform = None;
    project.visual.clips[0].start = 10.0;
    project.visual.clips[0].duration = 20.0;
    let mut plan = compile_project(project);
    let spectrum_index = plan
        .layers
        .iter()
        .position(|layer| matches!(layer.source, super::CompiledVisualSource::Spectrum2D { .. }))
        .expect("spectrum layer");
    stage_layer(&mut plan.layers[spectrum_index], 10.0, 30.0);
    plan.layers[spectrum_index].opacity = CompiledScalarProperty::authored(CompiledTrack::new(1.0));
    plan.layers[spectrum_index].opacity_contributions.clear();
    assert!(matches!(
        plan.layers[spectrum_index].source,
        super::CompiledVisualSource::Spectrum2D { ref band_signals, .. }
            if band_signals.len() == 24
    ));

    let mut samples = vec![0.0; 31];
    samples[5] = 0.2;
    samples[15] = 0.8;
    let prepared = PreparedScalarSignals::new(
        (0..24)
            .map(|_| {
                PreparedScalarSignal::new(0, 1_000_000_000, samples.clone()).expect("prepared band")
            })
            .collect(),
    );
    let context = EvaluationContext::new(&prepared);
    fn bands(frame: &super::EvaluatedFrame) -> Vec<f32> {
        match &frame.layers[0].source {
            super::EvaluatedSource::Spectrum2D { bands, .. } => bands.clone(),
            _ => panic!("expected Spectrum2D source"),
        }
    }
    let mut evaluations = Vec::new();
    for project_time in [15, 11, 18, 12, 15, 14, 5] {
        let frame = evaluate_with_context(
            &plan,
            &[super::ScheduledItem(spectrum_index)],
            project_time * 1_000_000_000,
            &context,
        )
        .expect("non-monotonic random-access evaluation");
        evaluations.push((project_time, bands(&frame)));
    }
    assert_eq!(evaluations[0].1, evaluations[4].1);
    assert_eq!(evaluations[0].1[0], 0.8);
    assert_eq!(evaluations[6].1[0], 0.2);
}

#[test]
fn nested_spectrum2d_samples_root_project_time() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let child = {
        let mut clip = project.visual.clips[0].clone();
        clip.id = "spectrum-child".into();
        clip.start = 2.0;
        clip.duration = 5.0;
        clip.source = VisualSource::Spectrum2D(Spectrum2D::default());
        clip.transform = None;
        clip
    };
    let mut group = child.clone();
    group.id = "spectrum-group".into();
    group.start = 10.0;
    group.duration = 10.0;
    group.source = VisualSource::Group(Group { clips: vec![child] });
    project.visual.clips = vec![group];
    let plan = compile_project(project);

    let prepared = PreparedScalarSignals::new(
        (0..24)
            .map(|_| {
                PreparedScalarSignal::new(0, 1_000_000_000, {
                    let mut samples = vec![0.0; 20];
                    samples[3] = 0.2;
                    samples[15] = 0.8;
                    samples
                })
                .expect("prepared band")
            })
            .collect(),
    );
    let frame = evaluate_with_context(
        &plan,
        &[super::ScheduledItem(0)],
        15_000_000_000,
        &EvaluationContext::new(&prepared),
    )
    .expect("nested Spectrum2D frame");
    let super::EvaluatedSource::Group { composition, .. } = &frame.layers[0].source else {
        panic!("expected Group");
    };
    let super::EvaluatedSource::Spectrum2D { bands, .. } = &composition.layers[0].source else {
        panic!("expected nested Spectrum2D");
    };
    assert_eq!(bands[0], 0.8);
}

#[test]
fn particle_system_compiles_and_evaluates_from_clip_local_time() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    project.visual.clips[0].source = VisualSource::ParticleSystem(ParticleSystem {
        seed: 99,
        emission: ParticleEmission {
            rate: 2.5,
            bursts: vec![ParticleBurst {
                time: 0.0,
                count: 2,
            }],
        },
        ..Default::default()
    });
    project.visual.clips[0].transform = None;
    let plan = compile_project(project);
    let particle_index = plan
        .layers
        .iter()
        .position(|layer| matches!(layer.source, super::CompiledVisualSource::ParticleSystem(_)))
        .expect("particle layer");
    let evaluate = |time| {
        evaluate_with_context(
            &plan,
            &[super::ScheduledItem(particle_index)],
            time,
            &EvaluationContext::new(&PreparedScalarSignals::empty()),
        )
        .expect("particle frame")
    };
    let direct = evaluate(1_500_000_000);
    let _ = evaluate(500_000_000);
    let _ = evaluate(2_000_000_000);
    let repeated = evaluate(1_500_000_000);
    let source = |frame: &super::EvaluatedFrame| match &frame.layers[0].source {
        super::EvaluatedSource::ParticleSystem {
            system,
            time_nanos,
            appearance,
        } => {
            let particles: Vec<_> = system
                .evaluated_particles_at_with_appearance(*time_nanos, *appearance)
                .collect();
            (particles.clone(), particles.len())
        }
        _ => panic!("expected particle source"),
    };
    assert_eq!(source(&direct), source(&repeated));
    assert_eq!(source(&direct).1, source(&repeated).1);
}

#[test]
fn nested_particles_keep_local_age_and_random_access_determinism() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let mut child = project.visual.clips[0].clone();
    child.id = "particle-child".into();
    child.start = 10.0;
    child.duration = 5.0;
    child.source = VisualSource::ParticleSystem(ParticleSystem {
        seed: 17,
        emission: ParticleEmission {
            rate: 2.0,
            ..Default::default()
        },
        ..Default::default()
    });
    child.transform = None;
    let mut group = child.clone();
    group.id = "particle-group".into();
    group.start = 10.0;
    group.duration = 20.0;
    group.source = VisualSource::Group(Group { clips: vec![child] });
    project.visual.clips = vec![group];
    let plan = compile_project(project);
    let prepared = PreparedScalarSignals::empty();
    let context = EvaluationContext::new(&prepared);
    let evaluate_nested = |time| {
        evaluate_with_context(&plan, &[super::ScheduledItem(0)], time, &context)
            .expect("nested particle frame")
    };
    let direct = evaluate_nested(22_000_000_000);
    let _ = evaluate_nested(20_000_000_000);
    let repeated = evaluate_nested(22_000_000_000);
    let particles = |frame: &super::EvaluatedFrame| {
        let super::EvaluatedSource::Group { composition, .. } = &frame.layers[0].source else {
            panic!("expected Group");
        };
        let super::EvaluatedSource::ParticleSystem {
            system,
            time_nanos,
            appearance,
        } = &composition.layers[0].source
        else {
            panic!("expected nested particles");
        };
        (
            *time_nanos,
            system
                .evaluated_particles_at_with_appearance(*time_nanos, *appearance)
                .collect::<Vec<_>>(),
        )
    };
    let (time, direct_particles) = particles(&direct);
    assert_eq!(time, 2_000_000_000);
    assert_eq!(direct_particles, particles(&repeated).1);
}

#[test]
fn compiled_identities_are_unique_across_compositions_with_local_ids() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let child = project.visual.clips[0].clone();
    let mut first = child.clone();
    first.id = "group-a".into();
    first.source = VisualSource::Group(Group {
        clips: vec![child.clone()],
    });
    first.transform = None;
    let mut second = first.clone();
    second.id = "group-b".into();
    project.visual.clips = vec![first, second];
    let plan = compile_project(project);
    let children = plan
        .layers
        .iter()
        .filter_map(|layer| match &layer.source {
            super::CompiledVisualSource::Group(composition) => Some(&composition.layers[0]),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].id, children[1].id);
    assert_ne!(children[0].compiled_identity, children[1].compiled_identity);
}

#[test]
fn particle_evaluation_emits_concrete_instances_for_the_renderer() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    project.visual.clips[0].source = VisualSource::ParticleSystem(ParticleSystem::default());
    project.visual.clips[0].transform = None;
    let plan = compile_project(project);
    let particle_index = plan
        .layers
        .iter()
        .position(|layer| matches!(layer.source, super::CompiledVisualSource::ParticleSystem(_)))
        .expect("particle layer");
    let super::CompiledVisualSource::ParticleSystem(compiled_system) =
        &plan.layers[particle_index].source
    else {
        panic!("expected compiled particle source");
    };
    let frame = evaluate(
        &plan,
        &[super::ScheduledItem(particle_index)],
        1_000_000_000,
    )
    .expect("particle frame");
    let super::EvaluatedSource::ParticleSystem { system, .. } = &frame.layers[0].source else {
        panic!("expected particle source");
    };
    assert!(std::sync::Arc::ptr_eq(system, compiled_system));
    assert_eq!(system.maximum_live_particles, 0);
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
        .filter_map(|event| (event.action == super::ScheduleAction::Activate).then_some(event.item))
        .collect::<Vec<_>>();
    let signals = PreparedScalarSignals::empty();
    let frame =
        evaluate_with_context(&plan, &active, 0, &EvaluationContext::new(&signals)).expect("frame");
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
            CompiledSignalTransform::Envelope(EnvelopeTransform::new(1_000_000_000, 1_000_000_000)),
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
fn nested_motion_blur_shutter_samples_shift_root_project_time() {
    let mut project = canonical_project();
    project.visual.transitions.clear();
    let mut child = project.visual.clips[0].clone();
    child.id = "nested-image".into();
    child.start = 0.0;
    child.duration = 3.0;
    child.effects.clear();
    let mut group = child.clone();
    group.id = "group".into();
    group.transform = None;
    group.source = VisualSource::Group(Group { clips: vec![child] });
    project.visual.clips = vec![group];
    let mut plan = compile_project(project);

    let super::CompiledVisualSource::Group(composition) = &mut plan.layers[0].source else {
        panic!("expected Group");
    };
    let composition = std::sync::Arc::get_mut(composition).expect("unique composition");
    let layer = &mut composition.layers[0];
    layer.transform.position = CompiledTrack::new(crate::domain::Point { x: 0.0, y: 0.0 });
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

    let signal =
        PreparedScalarSignal::new(0, 1_000_000_000, vec![0.0, 1.0, 2.0, 3.0]).expect("signal");
    let frame = evaluate_with_context(
        &plan,
        &[super::ScheduledItem(0)],
        1_500_000_000,
        &EvaluationContext::new(&PreparedScalarSignals::new(vec![signal])),
    )
    .expect("nested motion blur evaluation");
    let super::EvaluatedSource::Group { composition, .. } = &frame.layers[0].source else {
        panic!("expected evaluated Group");
    };
    assert!(matches!(
        composition.layers[0].effects.last(),
        Some(super::EvaluatedEffect::MotionBlur { radius, .. }) if *radius > 1.0
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
