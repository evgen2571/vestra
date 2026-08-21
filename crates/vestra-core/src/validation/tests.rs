use serde_json::{Value, json};

use super::{ResourceLimits, validate};
use crate::{
    Severity,
    project::{
        Effect, Interpolation, NormalizedKeyframe, NormalizedTrack, Project, TransitionDefinition,
        TransitionPlacement, TransitionPresentation,
    },
};

fn project(audio: Value) -> Project {
    serde_json::from_value(json!({
        "schema_version": 3,
        "output": {
            "path": "out.mp4", "width": 2, "height": 2,
            "frame_rate": "1/1", "background": "#000000",
            "quality": "balanced", "audio": true,
            "duration_mode": "explicit", "duration": 1.0
        },
        "assets": [
            {"id": "audio", "type": "audio", "source": "tone.wav"},
            {"id": "image", "type": "image", "source": "image.png"}
        ],
        "visual": {"clips": []},
        "audio": audio,
    }))
    .expect("test project schema")
}

fn clip(id: &str, asset: &str) -> Value {
    json!({"id": id, "asset": asset, "start": 0.0, "trim_start": 0.0})
}

fn track(id: &str, clips: Vec<Value>) -> Value {
    json!({"id": id, "gain": 1.0, "clips": clips})
}

fn codes(project: &Project) -> Vec<String> {
    validate(project, ResourceLimits::default())
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code.clone())
        .collect()
}

fn has(project: &Project, code: &str) -> bool {
    codes(project).iter().any(|item| item == code)
}

fn particle_project() -> Project {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../../examples/projects/animation-effects.json"
    ))
    .expect("fixture project");
    let clip = value["visual"]["clips"][0].as_object_mut().expect("clip");
    clip.insert(
        "source".to_owned(),
        json!({
            "type": "particle_system",
            "seed": 7,
            "emitter": {"type": "point", "position": {"x": 0.5, "y": 0.5}},
            "emission": {"rate": 2.5, "bursts": [{"time": 0.0, "count": 4}]},
            "particle": {"lifetime": 1.0, "initial_velocity": {"x": 0.1, "y": 0.0},
                "acceleration": {"x": 0.0, "y": 0.1}, "size": 1.0, "opacity": 1.0,
                "colour": "#ffffff", "rotation_degrees": 0.0, "angular_velocity_degrees": 10.0}
        }),
    );
    clip.remove("sizing");
    clip.remove("transform");
    value["visual"]["transitions"] = json!([]);
    serde_json::from_value(value).expect("particle project")
}

fn solid_clip(id: &str, start: f64, duration: f64) -> Value {
    json!({
        "id": id,
        "source": {"type": "solid_color", "colour": "#112233"},
        "start": start,
        "duration": duration,
        "layer": 0,
        "opacity": {"base_value": 1.0}
    })
}

fn group_clip(id: &str, clips: Vec<Value>) -> Value {
    json!({
        "id": id,
        "source": {"type": "group", "clips": clips},
        "start": 10.0,
        "duration": 5.0,
        "layer": 0,
        "opacity": {"base_value": 1.0}
    })
}

fn masked_project(masks: Value) -> Project {
    serde_json::from_value(json!({
        "schema_version": 4,
        "output": {
            "path": "out.mp4", "width": 32, "height": 32,
            "frame_rate": "1/1", "background": "#000000", "quality": "balanced",
            "audio": false, "duration_mode": "explicit", "duration": 1.0
        },
        "assets": [],
        "visual": {"clips": [{
            "id": "masked", "source": {"type": "solid_color", "colour": "#ff0000"},
            "start": 0.0, "duration": 1.0, "layer": 0,
            "opacity": {"base_value": 1.0}, "masks": masks
        }]}
    }))
    .expect("masked project")
}

fn shape_mask(id: &str, width: f64, strength: f64) -> Value {
    json!({
        "id": id,
        "input": {"type": "shape", "geometry": {"type": "ellipse", "width": width, "height": 16.0}, "fill": "#ffffff"},
        "strength": strength
    })
}

#[test]
fn masks_validate_ids_geometry_strength_and_dynamic_transform() {
    let project = masked_project(json!([
        shape_mask("duplicate", 16.0, 2.0),
        shape_mask("duplicate", 0.0, 1.0),
    ]));
    let codes = codes(&project);
    assert!(codes.iter().any(|code| code == "VESTRA-MASK-ID"));
    assert!(codes.iter().any(|code| code == "VESTRA-MASK-STRENGTH"));
    assert!(codes.iter().any(|code| code == "VESTRA-SHAPE-GEOMETRY"));

    let project = masked_project(json!([{
        "id": "oversized-feather",
        "input": {"type": "shape", "geometry": {"type": "ellipse", "width": 16.0, "height": 16.0}, "fill": "#ffffff"},
        "feather": 257.0
    }]));
    assert!(has(&project, "VESTRA-MASK-FEATHER"));

    let project = masked_project(json!([{
        "id": "animated",
        "input": {"type": "shape", "geometry": {"type": "rectangle", "width": 16.0, "height": 16.0}, "fill": "#ffffff"},
        "transform": {
            "position": {"base_value": {"x": 0.5, "y": 0.5}, "keyframes": [{
                "time": 0.0, "value": {"x": 0.6, "y": 0.5}, "interpolation": "linear"
            }]},
            "anchor": {"base_value": {"x": 0.5, "y": 0.5}, "keyframes": []},
            "scale": {"base_value": {"x": 1.0, "y": 1.0}, "keyframes": []},
            "rotation_degrees": {"base_value": 0.0, "keyframes": []}
        }
    }]));
    assert!(!has(&project, "VESTRA-MASK-STATIC-TRANSFORM"));
}

#[test]
fn masks_accept_line_geometry_as_rendered_coverage() {
    let project = masked_project(serde_json::json!([{
        "id": "line-mask",
        "input": {
            "type": "shape",
            "geometry": {"type": "line", "start": {"x": 0.0, "y": 0.0}, "end": {"x": 8.0, "y": 8.0}},
            "stroke": "#ffffff",
            "stroke_width": 2.0
        }
    }]));
    assert!(!has(&project, "VESTRA-MASK-SHAPE"));
}

fn grouped_project(clips: Vec<Value>) -> Project {
    let mut value = serde_json::json!({
        "schema_version": 3,
        "output": {
            "path": "out.mp4", "width": 2, "height": 2,
            "frame_rate": "1/1", "background": "#000000", "quality": "balanced",
            "audio": false, "duration_mode": "explicit", "duration": 20.0
        },
        "assets": [],
        "visual": {"clips": clips, "transitions": [], "flashes": [], "post_effects": []}
    });
    serde_json::from_value(value.take()).expect("grouped project")
}

fn crossfade(id: &str, outgoing: &str, incoming: &str) -> TransitionPlacement {
    TransitionPlacement {
        id: id.to_owned(),
        outgoing: outgoing.to_owned(),
        incoming: incoming.to_owned(),
        start: 10.0,
        duration: 1.0,
        definition: TransitionDefinition {
            outgoing: TransitionPresentation {
                opacity: Some(NormalizedTrack {
                    keyframes: vec![
                        NormalizedKeyframe {
                            progress: 0.0,
                            value: 1.0,
                            interpolation: Interpolation::Named(
                                crate::project::InterpolationName::Linear,
                            ),
                        },
                        NormalizedKeyframe {
                            progress: 1.0,
                            value: 0.0,
                            interpolation: Interpolation::Named(
                                crate::project::InterpolationName::Linear,
                            ),
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
                            interpolation: Interpolation::Named(
                                crate::project::InterpolationName::Linear,
                            ),
                        },
                        NormalizedKeyframe {
                            progress: 1.0,
                            value: 1.0,
                            interpolation: Interpolation::Named(
                                crate::project::InterpolationName::Linear,
                            ),
                        },
                    ],
                }),
                ..Default::default()
            },
        },
    }
}

fn transition_effect(id: &str) -> Effect {
    serde_json::from_value(json!({
        "type": "gaussian_blur",
        "id": id,
        "radius": {"base_value": 1.0}
    }))
    .expect("transition effect")
}

fn project_with_transition_effect_counts(
    authored: usize,
    outgoing: usize,
    incoming: usize,
) -> Project {
    let mut outgoing_clip = image_clip("a", "image-a", 10.0, 5.0);
    outgoing_clip["effects"] = json!(
        (0..authored)
            .map(|index| {
                json!({
                    "type": "gaussian_blur",
                    "id": format!("authored-{index}"),
                    "radius": {"base_value": 1.0}
                })
            })
            .collect::<Vec<_>>()
    );
    let incoming_clip = image_clip("b", "image-b", 10.0, 5.0);
    let mut project = asset_usage_project(vec![outgoing_clip, incoming_clip]);
    let mut placement = crossfade("effects", "a", "b");
    placement.definition.outgoing.effects = (0..outgoing)
        .map(|index| transition_effect(&format!("outgoing-{index}")))
        .collect();
    placement.definition.incoming.effects = (0..incoming)
        .map(|index| transition_effect(&format!("incoming-{index}")))
        .collect();
    project.visual.transitions = vec![placement];
    project
}

#[test]
fn transition_effect_count_combines_authored_and_outgoing_effects() {
    let project = project_with_transition_effect_counts(2, 2, 0);
    assert!(
        validate(
            &project,
            ResourceLimits {
                maximum_effects_per_clip: 4,
                ..ResourceLimits::default()
            }
        )
        .is_valid()
    );

    let project = project_with_transition_effect_counts(2, 3, 0);
    assert!(
        validate(
            &project,
            ResourceLimits {
                maximum_effects_per_clip: 4,
                ..ResourceLimits::default()
            }
        )
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-TRANSITION-EFFECTS")
    );
}

#[test]
fn transition_effect_count_checks_incoming_endpoint_independently() {
    let mut project = project_with_transition_effect_counts(2, 2, 3);
    project.visual.clips[1].effects = (0..2)
        .map(|index| transition_effect(&format!("incoming-authored-{index}")))
        .collect();
    assert!(
        validate(
            &project,
            ResourceLimits {
                maximum_effects_per_clip: 4,
                ..ResourceLimits::default()
            }
        )
        .diagnostics()
        .iter()
        .any(|diagnostic| {
            diagnostic
                .pointer
                .as_deref()
                .is_some_and(|pointer| pointer.ends_with("/incoming/effects"))
        })
    );
}

#[test]
fn root_group_transition_endpoints_are_valid_but_procedural_endpoints_are_not() {
    let mut project = grouped_project(vec![
        group_clip("group-a", vec![solid_clip("nested", 0.0, 5.0)]),
        group_clip("group-b", vec![solid_clip("nested", 0.0, 5.0)]),
    ]);
    project.visual.transitions = vec![crossfade("groups", "group-a", "group-b")];
    assert!(!has(&project, "VESTRA-TRANSITION-SOURCE"));

    let mut invalid = grouped_project(vec![
        solid_clip("solid", 10.0, 5.0),
        group_clip("group", vec![solid_clip("nested", 0.0, 5.0)]),
    ]);
    invalid.visual.transitions = vec![crossfade("solid-group", "solid", "group")];
    assert!(has(&invalid, "VESTRA-TRANSITION-SOURCE"));

    invalid.visual.transitions = vec![crossfade("nested-id", "nested", "group")];
    assert!(has(&invalid, "VESTRA-TRANSITION-CLIP"));
}

fn image_clip(id: &str, asset: &str, start: f64, duration: f64) -> Value {
    let example: Value = serde_json::from_str(include_str!(
        "../../../../examples/projects/animation-effects.json"
    ))
    .expect("image fixture project");
    json!({
        "id": id,
        "source": {"type": "image", "asset": asset},
        "start": start,
        "duration": duration,
        "layer": 0,
        "transform": example["visual"]["clips"][0]["transform"].clone(),
        "opacity": {"base_value": 1.0}
    })
}

fn asset_usage_project(clips: Vec<Value>) -> Project {
    serde_json::from_value(json!({
        "schema_version": 3,
        "output": {
            "path": "out.mp4", "width": 2, "height": 2,
            "frame_rate": "1/1", "background": "#000000", "quality": "balanced",
            "audio": false, "duration_mode": "explicit", "duration": 20.0
        },
        "assets": [
            {"id": "image-a", "type": "image", "source": "image-a.png"},
            {"id": "image-b", "type": "image", "source": "image-b.png"}
        ],
        "visual": {"clips": clips, "transitions": [], "flashes": [], "post_effects": []}
    }))
    .expect("asset usage project")
}

fn unused_asset_ids(project: &Project) -> Vec<String> {
    validate(project, ResourceLimits::default())
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == "VESTRA-ASSET-UNUSED")
        .filter_map(|diagnostic| diagnostic.related_id.clone())
        .collect()
}

#[test]
fn unused_asset_warning_traverses_group_descendants() {
    let direct = asset_usage_project(vec![group_clip(
        "group",
        vec![image_clip("child", "image-a", 0.0, 1.0)],
    )]);
    assert_eq!(unused_asset_ids(&direct), vec!["image-b"]);

    let nested = asset_usage_project(vec![group_clip(
        "group-a",
        vec![group_clip(
            "group-b",
            vec![image_clip("deep-child", "image-a", 0.0, 1.0)],
        )],
    )]);
    assert_eq!(unused_asset_ids(&nested), vec!["image-b"]);
}

#[test]
fn unused_asset_warning_preserves_flat_project_behavior() {
    let used = asset_usage_project(vec![image_clip("root", "image-a", 0.0, 1.0)]);
    assert_eq!(unused_asset_ids(&used), vec!["image-b"]);
}

#[test]
fn unused_asset_warning_traverses_owned_source_masks() {
    let project: Project = serde_json::from_value(json!({
        "schema_version": 4,
        "output": {
            "path": "out.mp4", "width": 2, "height": 2,
            "frame_rate": "1/1", "background": "#000000", "quality": "balanced",
            "audio": false, "duration_mode": "explicit", "duration": 1.0
        },
        "assets": [
            {"id": "font-a", "type": "font", "source": "font-a.ttf"},
            {"id": "image-a", "type": "image", "source": "image-a.png"},
            {"id": "image-unused", "type": "image", "source": "image-unused.png"}
        ],
        "visual": {"clips": [{
            "id": "owner", "source": {"type": "solid_color", "colour": "#ffffff"},
            "start": 0.0, "duration": 1.0, "layer": 0,
            "opacity": {"base_value": 1.0}, "masks": [{
                "id": "owned", "input": {"type": "source", "mode": "alpha",
                    "source": {"type": "group", "clips": [
                        {"id": "text", "source": {"type": "text", "text": "Vestra",
                            "font": "font-a", "font_size": 12.0, "fill": "#ffffff"},
                            "start": 0.0, "duration": 1.0, "layer": 0,
                            "opacity": {"base_value": 1.0}},
                        {"id": "image", "source": {"type": "image", "asset": "image-a"},
                            "start": 0.0, "duration": 1.0, "layer": 1,
                            "opacity": {"base_value": 1.0}}
                    ]}},
                "operation": "replace"
            }]
        }]}
    }))
    .expect("owned source mask project");

    assert_eq!(unused_asset_ids(&project), vec!["image-unused"]);
}

#[test]
fn groups_use_composition_local_ids_and_preserve_local_times() {
    let project = grouped_project(vec![
        group_clip("left", vec![solid_clip("x", 0.0, 10.0)]),
        group_clip("right", vec![solid_clip("x", 2.0, 10.0)]),
    ]);
    assert!(validate(&project, ResourceLimits::default()).is_valid());
    let crate::project::VisualSource::Group(group) = &project.visual.clips[0].source else {
        panic!("group source")
    };
    assert_eq!(group.clips[0].start, 0.0);
    assert_eq!(group.clips[0].duration, 10.0);
}

#[test]
fn groups_reject_duplicate_sibling_ids_with_nested_path() {
    let project = grouped_project(vec![group_clip(
        "group",
        vec![solid_clip("x", 0.0, 1.0), solid_clip("x", 2.0, 10.0)],
    )]);
    let report = validate(&project, ResourceLimits::default());
    let diagnostic = report
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == "VESTRA-CLIP-ID")
        .expect("duplicate id diagnostic");
    assert_eq!(
        diagnostic.pointer.as_deref(),
        Some("/visual/clips/0/source/clips/1/id")
    );
}

#[test]
fn group_depth_boundaries_are_explicit() {
    fn nested(depth: usize) -> Value {
        let mut value = solid_clip("leaf", 0.0, 1.0);
        for index in 0..depth {
            value = group_clip(&format!("group-{index}"), vec![value]);
        }
        value
    }

    for depth in [1, 31, 32] {
        assert!(
            validate(
                &grouped_project(vec![nested(depth)]),
                ResourceLimits::default()
            )
            .is_valid(),
            "depth {depth} should pass"
        );
    }
    let report = validate(
        &grouped_project(vec![nested(33)]),
        ResourceLimits::default(),
    );
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "VESTRA-GROUP-DEPTH"
            && diagnostic
                .pointer
                .as_deref()
                .is_some_and(|pointer| pointer.contains("/source/clips"))
    }));
}

#[test]
fn over_depth_groups_with_transitions_use_the_bounded_group_walk() {
    let mut nested = json!({
        "id": "leaf",
        "source": {"type": "group", "clips": []},
        "start": 0.0,
        "duration": 5.0,
        "layer": 0,
        "opacity": {"base_value": 1.0}
    });
    for depth in 0..40 {
        let nested_id = format!("nested-{depth}");
        let peer_id = format!("peer-{depth}");
        let peer = json!({
            "id": peer_id,
            "source": {"type": "group", "clips": []},
            "start": 0.0,
            "duration": 5.0,
            "layer": 1,
            "opacity": {"base_value": 1.0}
        });
        let mut placement = crossfade("local-transition", &nested_id, &peer_id);
        placement.start = 0.0;
        nested = json!({
            "id": nested_id,
            "source": {
                "type": "group",
                "clips": [nested, peer],
                "transitions": [serde_json::to_value(placement).expect("placement")]
            },
            "start": 0.0,
            "duration": 5.0,
            "layer": 0,
            "opacity": {"base_value": 1.0}
        });
    }

    let report = validate(&grouped_project(vec![nested]), ResourceLimits::default());
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-GROUP-DEPTH")
    );
}

#[test]
fn nested_groups_count_against_the_clip_limit() {
    let project = grouped_project(vec![group_clip(
        "group",
        vec![solid_clip("child", 0.0, 1.0)],
    )]);
    let limits = ResourceLimits {
        maximum_clips: 2,
        ..ResourceLimits::default()
    };
    assert!(validate(&project, limits).is_valid());
    let limits = ResourceLimits {
        maximum_clips: 1,
        ..ResourceLimits::default()
    };
    assert!(
        validate(&project, limits)
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-CLIPS")
    );
}

#[test]
fn nested_particle_validation_reuses_canonical_rules() {
    let mut project = particle_project();
    let particle = project.visual.clips[0].clone();
    project.visual.clips[0].source = crate::project::VisualSource::Group(crate::project::Group {
        clips: vec![particle],
        transitions: vec![],
    });
    let crate::project::VisualSource::Group(group) = &mut project.visual.clips[0].source else {
        panic!("group source")
    };
    let crate::project::VisualSource::ParticleSystem(system) = &mut group.clips[0].source else {
        panic!("particle source")
    };
    system.particle.lifetime = 0.0;
    assert!(codes(&project).contains(&"VESTRA-PARTICLE-LIFETIME".to_owned()));
}

#[test]
fn group_rejects_image_only_properties_but_keeps_group_properties_generic() {
    let mut value = group_clip("group", vec![solid_clip("child", 0.0, 1.0)]);
    value["sizing"] = json!({"mode": "fit"});
    let project = grouped_project(vec![value]);
    assert!(codes(&project).contains(&"VESTRA-GROUP-PROPERTIES".to_owned()));
}

#[test]
fn aggregate_particle_limits_include_particles_in_sibling_groups() {
    let source_project = particle_project();
    let child = source_project.visual.clips[0].clone();
    let mut child_value = serde_json::to_value(&child).expect("particle child JSON");
    fn remove_nulls(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.retain(|_, item| !item.is_null());
                for item in object.values_mut() {
                    remove_nulls(item);
                }
            }
            Value::Array(items) => {
                for item in items {
                    remove_nulls(item);
                }
            }
            _ => {}
        }
    }
    remove_nulls(&mut child_value);
    let mut left = group_clip("left", vec![child_value.clone()]);
    let mut right = group_clip("right", vec![child_value]);
    left["start"] = json!(0.0);
    right["start"] = json!(0.0);
    let project = grouped_project(vec![left, right]);
    let limits = ResourceLimits {
        maximum_total_live_particles: 1,
        ..ResourceLimits::default()
    };
    assert!(
        validate(&project, limits)
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-PARTICLES-TOTAL")
    );
}

#[test]
fn particle_system_validates_without_audio_or_clip_transforms() {
    let project = particle_project();
    let report = validate(&project, ResourceLimits::default());
    assert!(
        report
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Fatal),
        "{:?}",
        report.diagnostics()
    );
}

#[test]
fn particle_system_rejects_invalid_lifetime_and_transform() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.particle.lifetime = 0.0;
    project.visual.clips[0].transform = Some(crate::project::Transform {
        position: crate::project::Track::constant(crate::domain::Point { x: 0.5, y: 0.5 }),
        anchor: crate::project::Track::constant(crate::domain::Point { x: 0.5, y: 0.5 }),
        scale: crate::project::Track::constant(crate::domain::Point { x: 1.0, y: 1.0 }),
        rotation_degrees: crate::project::ScalarProperty::from_track(
            crate::project::Track::constant(0.0),
        ),
        component_modifiers: Default::default(),
    });
    let codes = codes(&project);
    assert!(codes.contains(&"VESTRA-PARTICLE-LIFETIME".to_owned()));
    assert!(codes.contains(&"VESTRA-PARTICLE-SYSTEM-TRANSFORM".to_owned()));
}

#[test]
fn particle_system_rejects_positive_lifetime_that_quantizes_to_zero() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.particle.lifetime = 0.000_000_000_1;
    assert!(codes(&project).contains(&"VESTRA-PARTICLE-LIFETIME".to_owned()));
}

#[test]
fn particle_system_rejects_quantized_zero_lifetime_with_burst() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.particle.lifetime = 0.000_000_000_1;
    system.emission.bursts = vec![crate::project::ParticleBurst {
        time: 0.0,
        count: 1,
    }];
    assert!(codes(&project).contains(&"VESTRA-PARTICLE-LIFETIME".to_owned()));
}

#[test]
fn base_lifetime_remains_required_when_a_valid_range_is_present() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.particle.lifetime = 0.0;
    system.particle.lifetime_range = Some(crate::project::ScalarRange { min: 1.0, max: 3.0 });
    let report = validate(&project, ResourceLimits::default());
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "VESTRA-PARTICLE-LIFETIME"
            && diagnostic.pointer.as_deref() == Some("/visual/clips/0/source/particle/lifetime")
    }));
}

#[test]
fn lifetime_range_is_validated_separately_from_the_base_lifetime() {
    let mut project = particle_project();
    if let crate::project::VisualSource::ParticleSystem(system) =
        &mut project.visual.clips[0].source
    {
        system.particle.lifetime = 2.0;
        system.particle.lifetime_range = Some(crate::project::ScalarRange { min: 3.0, max: 1.0 });
    } else {
        panic!("particle source");
    }
    let report = validate(&project, ResourceLimits::default());
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "VESTRA-PARTICLE-RANGE"
            && diagnostic.pointer.as_deref()
                == Some("/visual/clips/0/source/particle/lifetime_range")
    }));

    if let crate::project::VisualSource::ParticleSystem(system) =
        &mut project.visual.clips[0].source
    {
        system.particle.lifetime_range = Some(crate::project::ScalarRange { min: 1.0, max: 3.0 });
    } else {
        panic!("particle source");
    }
    assert!(accepted(&project, ResourceLimits::default()));
}

#[test]
fn particle_live_limit_uses_lifetime_range_maximum_not_its_minimum() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.emission.rate = 1.0;
    system.particle.lifetime_range = Some(crate::project::ScalarRange {
        min: 1.0,
        max: 10.0,
    });
    let limits = ResourceLimits {
        maximum_live_particles_per_system: 5,
        ..ResourceLimits::default()
    };
    assert!(
        validate(&project, limits)
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-PARTICLES")
    );
}

#[test]
fn rectangle_component_diagnostics_use_json_pointer_segments() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.emitter = crate::project::ParticleEmitter::Rectangle {
        center: crate::domain::Point { x: 0.5, y: 0.5 },
        size: crate::domain::Point { x: -1.0, y: 0.25 },
    };
    assert!(
        validate(&project, ResourceLimits::default())
            .diagnostics()
            .iter()
            .any(|diagnostic| {
                diagnostic.pointer.as_deref() == Some("/visual/clips/0/source/emitter/size/x")
            })
    );
}

#[test]
fn particle_system_custom_live_limit_is_enforced() {
    let project = particle_project();
    let limits = ResourceLimits {
        maximum_live_particles_per_system: 2,
        ..ResourceLimits::default()
    };
    assert!(
        validate(&project, limits)
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-PARTICLES")
    );
}

#[test]
fn particle_emitter_accepts_finite_off_canvas_coordinates() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.emitter = crate::project::ParticleEmitter::Point {
        position: crate::domain::Point { x: -3.0, y: 4.0 },
    };
    assert!(!codes(&project).contains(&"VESTRA-PARTICLE-NUMERIC".to_owned()));
}

#[test]
fn particle_emitter_rejects_non_finite_coordinates() {
    for (x, y, field) in [
        (f64::NAN, 0.5, "x"),
        (0.5, f64::NAN, "y"),
        (f64::INFINITY, 0.5, "x"),
        (0.5, f64::NEG_INFINITY, "y"),
    ] {
        let mut project = particle_project();
        let crate::project::VisualSource::ParticleSystem(system) =
            &mut project.visual.clips[0].source
        else {
            panic!("particle source")
        };
        system.emitter = crate::project::ParticleEmitter::Point {
            position: crate::domain::Point { x, y },
        };
        let report = validate(&project, ResourceLimits::default());
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.pointer
                    == Some(format!("/visual/clips/0/source/emitter/position/{field}")))
        );
    }
}

#[test]
fn particle_emitters_and_ranges_validate_their_bounds() {
    let mut project = particle_project();
    {
        let crate::project::VisualSource::ParticleSystem(system) =
            &mut project.visual.clips[0].source
        else {
            panic!("particle source")
        };
        system.emitter = crate::project::ParticleEmitter::Rectangle {
            center: crate::domain::Point { x: 0.5, y: 0.5 },
            size: crate::domain::Point { x: 0.0, y: 0.25 },
        };
        system.particle.lifetime_range = Some(crate::project::ScalarRange { min: 1.0, max: 3.0 });
        system.particle.size_range = Some(crate::project::ScalarRange {
            min: 0.01,
            max: 0.1,
        });
    }
    assert!(accepted(&project, ResourceLimits::default()));

    if let crate::project::VisualSource::ParticleSystem(system) =
        &mut project.visual.clips[0].source
    {
        system.emitter = crate::project::ParticleEmitter::Circle {
            center: crate::domain::Point { x: 0.5, y: 0.5 },
            inner_radius: 0.8,
            outer_radius: 0.2,
        };
    }
    assert!(codes(&project).contains(&"VESTRA-PARTICLE-EMITTER-RADIUS".to_owned()));
    if let crate::project::VisualSource::ParticleSystem(system) =
        &mut project.visual.clips[0].source
    {
        system.emitter = crate::project::ParticleEmitter::default();
        system.particle.lifetime_range = Some(crate::project::ScalarRange { min: 3.0, max: 1.0 });
    }
    assert!(codes(&project).contains(&"VESTRA-PARTICLE-RANGE".to_owned()));
}

#[test]
fn sequential_particle_clips_do_not_consume_aggregate_budget_twice() {
    let mut project = particle_project();
    project.visual.clips[0].start = 0.0;
    project.visual.clips[0].duration = 10.0;
    let mut next = project.visual.clips[0].clone();
    next.id = "particles-2".to_owned();
    next.start = 10.0;
    project.visual.clips.push(next);
    let limits = ResourceLimits {
        maximum_total_live_particles: 10,
        ..ResourceLimits::default()
    };
    assert!(accepted(&project, limits));
}

#[test]
fn overlapping_particle_clips_consume_aggregate_budget() {
    let mut project = particle_project();
    project.visual.clips[0].duration = 20.0;
    let mut next = project.visual.clips[0].clone();
    next.id = "particles-2".to_owned();
    next.start = 10.0;
    project.visual.clips.push(next);
    let limits = ResourceLimits {
        maximum_total_live_particles: 7,
        ..ResourceLimits::default()
    };
    assert!(
        validate(&project, limits)
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-PARTICLES-TOTAL")
    );
}

#[test]
fn particle_clip_end_and_start_at_same_time_do_not_overlap() {
    let mut project = particle_project();
    project.visual.clips[0].duration = 10.0;
    let mut next = project.visual.clips[0].clone();
    next.id = "particles-2".to_owned();
    next.start = 10.0;
    project.visual.clips.push(next);
    let limits = ResourceLimits {
        maximum_total_live_particles: 7,
        ..ResourceLimits::default()
    };
    assert!(accepted(&project, limits));
}

fn accepted(project: &Project, limits: ResourceLimits) -> bool {
    validate(project, limits)
        .diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Fatal)
}

fn example_project() -> Project {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/projects/animation-effects.json"
    );
    Project::from_json(&std::fs::read_to_string(path).expect("project")).expect("schema")
}

#[test]
fn pure_validation_does_not_access_referenced_assets() {
    let mut project = example_project();
    project.assets[0].source = "definitely-not-present.png".to_owned();

    let report = validate(&project, ResourceLimits::default());

    assert!(report.is_valid(), "{:?}", report.diagnostics());
}

#[test]
fn pure_validation_reports_schema_semantics_without_a_backend() {
    let mut project = example_project();
    project.visual.clips[0].id.clear();

    let report = validate(&project, ResourceLimits::default());

    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-CLIP-ID")
    );
}

#[test]
fn master_signal_requires_authored_audio_and_reports_the_source_path() {
    let mut project = example_project();
    project.visual.clips[0].opacity.modifiers = vec![crate::project::ScalarModifier {
        operation: crate::project::ScalarModifierOperation::Add,
        signal: crate::project::ScalarSignal {
            source: crate::project::ScalarSignalSource::Audio {
                tap: crate::project::AudioAnalysisTap::Master,
                feature: crate::project::AudioScalarFeature::Rms,
            },
            transforms: vec![],
        },
    }];
    let report = validate(&project, ResourceLimits::default());
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "VESTRA-SIGNAL-MASTER-AUDIO"
            && diagnostic.pointer.as_deref()
                == Some("/visual/clips/0/opacity/modifiers/0/signal/source")
    }));
}

#[test]
fn spectrum2d_requires_authored_audio_during_project_validation() {
    let mut project = example_project();
    project.visual.clips[0].source =
        crate::project::VisualSource::Spectrum2D(crate::project::Spectrum2D::default());

    let report = validate(&project, ResourceLimits::default());

    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "VESTRA-SPECTRUM2D-MASTER-AUDIO"
            && diagnostic.pointer.as_deref() == Some("/visual/clips/0/source")
    }));
}

#[test]
fn spectrum2d_accepts_authored_audio_during_project_validation() {
    let mut project = example_project();
    project.visual.clips[0].source =
        crate::project::VisualSource::Spectrum2D(crate::project::Spectrum2D::default());
    project.audio = Some(
        serde_json::from_value(json!({
            "tracks": [track("music", vec![clip("clip", "audio")])]
        }))
        .expect("audio timeline"),
    );

    let report = validate(&project, ResourceLimits::default());

    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-SPECTRUM2D-MASTER-AUDIO")
    );
}

#[test]
fn signal_validation_points_to_the_invalid_transform_field() {
    let mut project = example_project();
    project.visual.clips[0].opacity.modifiers = vec![crate::project::ScalarModifier {
        operation: crate::project::ScalarModifierOperation::Add,
        signal: crate::project::ScalarSignal {
            source: crate::project::ScalarSignalSource::Audio {
                tap: crate::project::AudioAnalysisTap::Master,
                feature: crate::project::AudioScalarFeature::Rms,
            },
            transforms: vec![crate::project::SignalTransform::Envelope {
                attack: 0.02,
                release: -0.18,
            }],
        },
    }];

    let report = validate(&project, ResourceLimits::default());
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "VESTRA-SIGNAL-TRANSFORM"
            && diagnostic.pointer.as_deref()
                == Some("/visual/clips/0/opacity/modifiers/0/signal/transforms/0/release")
    }));
}

#[test]
fn component_signal_validation_uses_the_component_modifier_array_pointer() {
    let fields = ["position_x", "position_y", "scale_x", "scale_y"];
    for field in fields {
        let mut project = example_project();
        let modifiers = match field {
            "position_x" => {
                &mut project.visual.clips[0]
                    .transform
                    .as_mut()
                    .expect("transform")
                    .component_modifiers
                    .position_x
            }
            "position_y" => {
                &mut project.visual.clips[0]
                    .transform
                    .as_mut()
                    .expect("transform")
                    .component_modifiers
                    .position_y
            }
            "scale_x" => {
                &mut project.visual.clips[0]
                    .transform
                    .as_mut()
                    .expect("transform")
                    .component_modifiers
                    .scale_x
            }
            "scale_y" => {
                &mut project.visual.clips[0]
                    .transform
                    .as_mut()
                    .expect("transform")
                    .component_modifiers
                    .scale_y
            }
            _ => unreachable!("known transform component"),
        };
        modifiers.push(crate::project::ScalarModifier {
            operation: crate::project::ScalarModifierOperation::Add,
            signal: crate::project::ScalarSignal {
                source: crate::project::ScalarSignalSource::Audio {
                    tap: crate::project::AudioAnalysisTap::Master,
                    feature: crate::project::AudioScalarFeature::BandEnergy {
                        min_hz: 160.0,
                        max_hz: 40.0,
                    },
                },
                transforms: vec![],
            },
        });
        let expected = format!(
            "/visual/clips/0/transform/component_modifiers/{field}/0/signal/source/feature/min_hz"
        );
        assert!(
            validate(&project, ResourceLimits::default())
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "VESTRA-SIGNAL-BAND"
                    && diagnostic.pointer.as_deref() == Some(expected.as_str()))
        );
    }
}

#[test]
fn master_signal_accepts_authored_silence_even_when_output_audio_is_disabled() {
    let mut project = example_project();
    project.audio = Some(
        serde_json::from_value(json!({"tracks": [track("music", vec![clip("clip", "audio")])]}))
            .expect("audio timeline"),
    );
    project.output.audio = false;
    project.audio.as_mut().expect("audio").tracks[0].mute = true;
    project.visual.clips[0].opacity.modifiers = vec![crate::project::ScalarModifier {
        operation: crate::project::ScalarModifierOperation::Add,
        signal: crate::project::ScalarSignal {
            source: crate::project::ScalarSignalSource::Audio {
                tap: crate::project::AudioAnalysisTap::Master,
                feature: crate::project::AudioScalarFeature::Peak,
            },
            transforms: vec![],
        },
    }];

    assert!(
        !validate(&project, ResourceLimits::default())
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-SIGNAL-MASTER-AUDIO")
    );
}

#[test]
fn signal_validation_covers_band_and_transform_contracts_at_field_paths() {
    let cases = [
        (
            crate::project::AudioScalarFeature::BandEnergy {
                min_hz: 160.0,
                max_hz: 40.0,
            },
            vec![],
            "/source/feature/min_hz",
        ),
        (
            crate::project::AudioScalarFeature::Rms,
            vec![crate::project::SignalTransform::Remap {
                input_min: 1.0,
                input_max: 1.0,
                output_start: 0.0,
                output_end: 1.0,
            }],
            "/transforms/0/input_min",
        ),
        (
            crate::project::AudioScalarFeature::Rms,
            vec![crate::project::SignalTransform::Clamp { min: 1.0, max: 0.0 }],
            "/transforms/0/min",
        ),
        (
            crate::project::AudioScalarFeature::Rms,
            vec![crate::project::SignalTransform::ResponseCurve {
                x1: 0.8,
                y1: 0.0,
                x2: 0.2,
                y2: 1.0,
            }],
            "/transforms/0/x2",
        ),
    ];
    for (feature, transforms, suffix) in cases {
        let mut project = example_project();
        project.visual.clips[0].opacity.modifiers = vec![crate::project::ScalarModifier {
            operation: crate::project::ScalarModifierOperation::Add,
            signal: crate::project::ScalarSignal {
                source: crate::project::ScalarSignalSource::Audio {
                    tap: crate::project::AudioAnalysisTap::Master,
                    feature,
                },
                transforms,
            },
        }];
        let pointer = format!("/visual/clips/0/opacity/modifiers/0/signal{suffix}");
        assert!(
            validate(&project, ResourceLimits::default())
                .diagnostics()
                .iter()
                .any(|diagnostic| {
                    (diagnostic.code == "VESTRA-SIGNAL-BAND"
                        || diagnostic.code == "VESTRA-SIGNAL-TRANSFORM")
                        && diagnostic.pointer.as_deref() == Some(pointer.as_str())
                },)
        );
    }
}

#[test]
fn audio_validation_enforces_ids_assets_and_global_clip_identity() {
    let valid = project(json!({"tracks": [track("music", vec![clip("clip-a", "audio")])]}));
    assert!(accepted(&valid, ResourceLimits::default()));

    let duplicate_track = project(json!({"tracks": [
        track("music", vec![]), track("music", vec![])
    ]}));
    assert!(has(&duplicate_track, "VESTRA-AUDIO-TRACK-ID"));

    let duplicate_same_track = project(json!({"tracks": [track("music", vec![
        clip("clip-a", "audio"), clip("clip-a", "audio")
    ])]}));
    assert!(has(&duplicate_same_track, "VESTRA-AUDIO-CLIP-ID"));

    let duplicate_cross_track = project(json!({"tracks": [
        track("music", vec![clip("clip-a", "audio")]),
        track("sfx", vec![clip("clip-a", "audio")])
    ]}));
    assert!(has(&duplicate_cross_track, "VESTRA-AUDIO-CLIP-ID"));

    let missing_asset =
        project(json!({"tracks": [track("music", vec![clip("clip-a", "missing")])]}));
    assert!(has(&missing_asset, "VESTRA-AUDIO-ASSET"));

    let image_asset = project(json!({"tracks": [track("music", vec![clip("clip-a", "image")])]}));
    assert!(has(&image_asset, "VESTRA-AUDIO-ASSET-TYPE"));
}

#[test]
fn audio_validation_accepts_linear_gain_and_rejects_invalid_gain_at_each_layer() {
    let valid = project(json!({"tracks": [json!({
        "id": "music", "gain": 1.5,
        "clips": [json!({"id": "clip-a", "asset": "audio", "start": 0.0,
            "trim_start": 0.0, "gain": 2.0})]
    })]}));
    assert!(accepted(&valid, ResourceLimits::default()));

    let negative_track =
        project(json!({"tracks": [json!({"id": "music", "gain": -0.1, "clips": []})]}));
    assert!(has(&negative_track, "VESTRA-AUDIO-TRACK-GAIN"));
    let negative_clip = project(json!({"tracks": [json!({"id": "music", "clips": [json!({
        "id": "clip-a", "asset": "audio", "start": 0.0, "trim_start": 0.0, "gain": -0.1
    })]})]}));
    assert!(has(&negative_clip, "VESTRA-AUDIO-CLIP-GAIN"));

    let mut non_finite = valid;
    non_finite.audio.as_mut().expect("audio").tracks[0].gain = f64::NAN;
    non_finite.audio.as_mut().expect("audio").tracks[0].clips[0].gain = f64::INFINITY;
    let non_finite_codes = codes(&non_finite);
    assert!(
        non_finite_codes
            .iter()
            .any(|code| code == "VESTRA-AUDIO-TRACK-GAIN")
    );
    assert!(
        non_finite_codes
            .iter()
            .any(|code| code == "VESTRA-AUDIO-CLIP-GAIN")
    );
}

#[test]
fn audio_gain_automation_validates_order_values_and_audibility_independently() {
    let automation = |keyframes: Value| {
        json!({"tracks": [json!({"id": "music", "clips": [json!({
            "id": "clip-a", "asset": "audio", "start": 0.0, "trim_start": 0.0,
            "gain_automation": {"keyframes": keyframes}
        })]})]})
    };
    let valid = project(automation(json!([
        {"time": 0.0, "gain": 1.5, "interpolation": "linear"},
        {"time": 0.25, "gain": 0.0, "interpolation": "hold"},
        {"time": 0.5, "gain": 2.0}
    ])));
    assert!(accepted(&valid, ResourceLimits::default()));
    for (keyframes, code) in [
        (json!([]), "VESTRA-AUDIO-AUTOMATION"),
        (
            json!([{"time": 0.1, "gain": 1.0}]),
            "VESTRA-AUDIO-AUTOMATION-TIME",
        ),
        (
            json!([{"time": 0.0, "gain": 1.0}, {"time": 0.0, "gain": 1.0}]),
            "VESTRA-AUDIO-AUTOMATION-TIME",
        ),
        (
            json!([{"time": 0.0, "gain": -1.0}]),
            "VESTRA-AUDIO-AUTOMATION-GAIN",
        ),
    ] {
        assert!(has(&project(automation(keyframes)), code));
    }
    let mut non_finite = valid.clone();
    non_finite.audio.as_mut().expect("audio").tracks[0].clips[0]
        .gain_automation
        .as_mut()
        .expect("automation")
        .keyframes[0]
        .gain = f64::NAN;
    assert!(has(&non_finite, "VESTRA-AUDIO-AUTOMATION-GAIN"));
    let mut muted = project(automation(json!([{"time": 0.1, "gain": 1.0}])));
    muted.audio.as_mut().expect("audio").tracks[0].mute = true;
    muted.output.audio = false;
    assert!(has(&muted, "VESTRA-AUDIO-AUTOMATION-TIME"));
}

#[test]
fn audio_gain_keyframe_limit_is_inclusive() {
    let limits = ResourceLimits {
        maximum_audio_gain_keyframes: 2,
        ..ResourceLimits::default()
    };
    let points = |count| {
        (0..count)
            .map(|index| json!({"time": index as f64, "gain": 1.0}))
            .collect::<Vec<_>>()
    };
    assert!(accepted(
        &project(
            json!({"tracks": [json!({"id": "music", "clips": [json!({"id": "clip", "asset": "audio", "start": 0.0, "trim_start": 0.0, "gain_automation": {"keyframes": points(2)}})]})]})
        ),
        limits
    ));
    let over = project(
        json!({"tracks": [json!({"id": "music", "clips": [json!({"id": "clip", "asset": "audio", "start": 0.0, "trim_start": 0.0, "gain_automation": {"keyframes": points(3)}})]})]}),
    );
    assert!(
        validate(&over, limits)
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "VESTRA-LIMIT-AUDIO-GAIN-KEYFRAMES")
    );
}

#[test]
fn audio_overlap_and_audibility_flags_do_not_bypass_semantic_validation() {
    let same_track_overlap = project(json!({"tracks": [json!({"id": "music", "clips": [
        json!({"id": "clip-a", "asset": "audio", "start": 0.0, "trim_start": 0.0}),
        json!({"id": "clip-b", "asset": "audio", "start": 0.5, "trim_start": 0.0})
    ]})]}));
    assert!(accepted(&same_track_overlap, ResourceLimits::default()));
    let cross_track_overlap = project(json!({"tracks": [
        track("music", vec![clip("clip-a", "audio")]),
        track("sfx", vec![clip("clip-b", "audio")])
    ]}));
    assert!(accepted(&cross_track_overlap, ResourceLimits::default()));

    for audio in [
        json!({"tracks": [json!({"id": "music", "mute": true, "clips": [json!({
            "id": "clip-a", "asset": "missing", "start": 0.0, "trim_start": 0.0, "mute": true
        })]})]}),
        json!({"tracks": [json!({"id": "music", "gain": 0.0, "clips": [json!({
            "id": "clip-a", "asset": "missing", "start": 0.0, "trim_start": 0.0, "gain": 0.0
        })]})]}),
    ] {
        let invalid = project(audio);
        assert!(has(&invalid, "VESTRA-AUDIO-ASSET"));
    }

    let mut output_disabled =
        project(json!({"tracks": [track("music", vec![clip("clip-a", "missing")])]}));
    output_disabled.output.audio = false;
    assert!(has(&output_disabled, "VESTRA-AUDIO-ASSET"));
}

#[test]
fn audio_complexity_limits_are_inclusive_and_deterministic() {
    let limits = ResourceLimits::default();
    let tracks = (0..limits.maximum_audio_tracks)
        .map(|index| track(&format!("track-{index}"), vec![]))
        .collect::<Vec<_>>();
    assert!(accepted(&project(json!({"tracks": tracks})), limits));
    let tracks = (0..=limits.maximum_audio_tracks)
        .map(|index| track(&format!("track-{index}"), vec![]))
        .collect::<Vec<_>>();
    assert!(has(
        &project(json!({"tracks": tracks})),
        "VESTRA-LIMIT-AUDIO-TRACKS"
    ));

    let clips = (0..limits.maximum_audio_clips)
        .map(|index| clip(&format!("clip-{index}"), "audio"))
        .collect::<Vec<_>>();
    assert!(accepted(
        &project(json!({"tracks": [track("music", clips)]})),
        limits
    ));
    let clips = (0..=limits.maximum_audio_clips)
        .map(|index| clip(&format!("clip-{index}"), "audio"))
        .collect::<Vec<_>>();
    assert!(has(
        &project(json!({"tracks": [track("music", clips)]})),
        "VESTRA-LIMIT-AUDIO-CLIPS"
    ));
}

#[test]
fn audio_effect_ids_are_validated_deterministically_per_collection() {
    let invalid = project(json!({
        "effects": [
            {"id": "", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1},
            {"id": "eq", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1},
            {"id": "eq", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}
        ],
        "tracks": [{
            "id": "track", "effects": [{"id": "", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}, {"id": "track-eq", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}, {"id": "track-eq", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}],
            "clips": [{"id": "clip", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}, {"id": "clip-eq", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}, {"id": "clip-eq", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}]}]
        }]
    }));
    assert_eq!(
        codes(&invalid)
            .iter()
            .filter(|code| *code == "VESTRA-AUDIO-EFFECT-ID")
            .count(),
        6
    );

    let allowed = project(json!({
        "tracks": [
            {"id": "track-a", "clips": [{"id": "clip-a", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "same", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}]}]},
            {"id": "track-b", "clips": [{"id": "clip-b", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "same", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}]}]}
        ]
    }));
    assert!(!has(&allowed, "VESTRA-AUDIO-EFFECT-ID"));
}

#[test]
fn playback_speed_is_clip_only_and_uses_declared_bounds() {
    let valid = project(
        json!({"tracks": [{"id": "track", "clips": [{"id": "clip", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "speed", "type": "playback_speed", "rate": 0.25}]}]}]}),
    );
    assert!(!has(&valid, "VESTRA-AUDIO-EFFECT-SCOPE"));
    let invalid = project(
        json!({"effects": [{"id": "speed", "type": "playback_speed", "rate": 2}], "tracks": [{"id": "track", "effects": [{"id": "speed-track", "type": "playback_speed", "rate": 2}], "clips": []}]}),
    );
    assert_eq!(
        codes(&invalid)
            .iter()
            .filter(|code| *code == "VESTRA-AUDIO-EFFECT-SCOPE")
            .count(),
        2
    );
    for rate in [0.0, 0.249, 4.001] {
        let out_of_range = project(json!({
            "tracks": [{"id": "track", "clips": [{"id": "clip", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "speed", "type": "playback_speed", "rate": rate}]}]}]
        }));
        assert!(has(&out_of_range, "VESTRA-AUDIO-EFFECT-PARAMETER"));
    }
}
