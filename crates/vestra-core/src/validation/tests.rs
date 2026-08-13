use serde_json::{Value, json};

use super::{ResourceLimits, validate};
use crate::{Severity, project::Project};

fn project(audio: Value) -> Project {
    serde_json::from_value(json!({
        "schema_version": 2,
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
    assert!(codes.contains(&"MVP-PARTICLE-LIFETIME".to_owned()));
    assert!(codes.contains(&"MVP-PARTICLE-SYSTEM-TRANSFORM".to_owned()));
}

#[test]
fn particle_system_rejects_positive_lifetime_that_quantizes_to_zero() {
    let mut project = particle_project();
    let crate::project::VisualSource::ParticleSystem(system) = &mut project.visual.clips[0].source
    else {
        panic!("particle source")
    };
    system.particle.lifetime = 0.000_000_000_1;
    assert!(codes(&project).contains(&"MVP-PARTICLE-LIFETIME".to_owned()));
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
    assert!(codes(&project).contains(&"MVP-PARTICLE-LIFETIME".to_owned()));
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
        diagnostic.code == "MVP-PARTICLE-LIFETIME"
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
        diagnostic.code == "MVP-PARTICLE-RANGE"
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
            .any(|diagnostic| diagnostic.code == "MVP-LIMIT-PARTICLES")
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
            .any(|diagnostic| diagnostic.code == "MVP-LIMIT-PARTICLES")
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
    assert!(!codes(&project).contains(&"MVP-PARTICLE-NUMERIC".to_owned()));
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
    assert!(codes(&project).contains(&"MVP-PARTICLE-EMITTER-RADIUS".to_owned()));
    if let crate::project::VisualSource::ParticleSystem(system) =
        &mut project.visual.clips[0].source
    {
        system.emitter = crate::project::ParticleEmitter::default();
        system.particle.lifetime_range = Some(crate::project::ScalarRange { min: 3.0, max: 1.0 });
    }
    assert!(codes(&project).contains(&"MVP-PARTICLE-RANGE".to_owned()));
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
            .any(|diagnostic| diagnostic.code == "MVP-LIMIT-PARTICLES-TOTAL")
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
            .any(|diagnostic| diagnostic.code == "MVP-CLIP-ID")
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
        diagnostic.code == "MVP-SIGNAL-MASTER-AUDIO"
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
        diagnostic.code == "MVP-SPECTRUM2D-MASTER-AUDIO"
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
            .any(|diagnostic| diagnostic.code == "MVP-SPECTRUM2D-MASTER-AUDIO")
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
        diagnostic.code == "MVP-SIGNAL-TRANSFORM"
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
                .any(|diagnostic| diagnostic.code == "MVP-SIGNAL-BAND"
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
            .any(|diagnostic| diagnostic.code == "MVP-SIGNAL-MASTER-AUDIO")
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
                    (diagnostic.code == "MVP-SIGNAL-BAND"
                        || diagnostic.code == "MVP-SIGNAL-TRANSFORM")
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
    assert!(has(&duplicate_track, "MVP-AUDIO-TRACK-ID"));

    let duplicate_same_track = project(json!({"tracks": [track("music", vec![
        clip("clip-a", "audio"), clip("clip-a", "audio")
    ])]}));
    assert!(has(&duplicate_same_track, "MVP-AUDIO-CLIP-ID"));

    let duplicate_cross_track = project(json!({"tracks": [
        track("music", vec![clip("clip-a", "audio")]),
        track("sfx", vec![clip("clip-a", "audio")])
    ]}));
    assert!(has(&duplicate_cross_track, "MVP-AUDIO-CLIP-ID"));

    let missing_asset =
        project(json!({"tracks": [track("music", vec![clip("clip-a", "missing")])]}));
    assert!(has(&missing_asset, "MVP-AUDIO-ASSET"));

    let image_asset = project(json!({"tracks": [track("music", vec![clip("clip-a", "image")])]}));
    assert!(has(&image_asset, "MVP-AUDIO-ASSET-TYPE"));
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
    assert!(has(&negative_track, "MVP-AUDIO-TRACK-GAIN"));
    let negative_clip = project(json!({"tracks": [json!({"id": "music", "clips": [json!({
        "id": "clip-a", "asset": "audio", "start": 0.0, "trim_start": 0.0, "gain": -0.1
    })]})]}));
    assert!(has(&negative_clip, "MVP-AUDIO-CLIP-GAIN"));

    let mut non_finite = valid;
    non_finite.audio.as_mut().expect("audio").tracks[0].gain = f64::NAN;
    non_finite.audio.as_mut().expect("audio").tracks[0].clips[0].gain = f64::INFINITY;
    let non_finite_codes = codes(&non_finite);
    assert!(
        non_finite_codes
            .iter()
            .any(|code| code == "MVP-AUDIO-TRACK-GAIN")
    );
    assert!(
        non_finite_codes
            .iter()
            .any(|code| code == "MVP-AUDIO-CLIP-GAIN")
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
        (json!([]), "MVP-AUDIO-AUTOMATION"),
        (
            json!([{"time": 0.1, "gain": 1.0}]),
            "MVP-AUDIO-AUTOMATION-TIME",
        ),
        (
            json!([{"time": 0.0, "gain": 1.0}, {"time": 0.0, "gain": 1.0}]),
            "MVP-AUDIO-AUTOMATION-TIME",
        ),
        (
            json!([{"time": 0.0, "gain": -1.0}]),
            "MVP-AUDIO-AUTOMATION-GAIN",
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
    assert!(has(&non_finite, "MVP-AUDIO-AUTOMATION-GAIN"));
    let mut muted = project(automation(json!([{"time": 0.1, "gain": 1.0}])));
    muted.audio.as_mut().expect("audio").tracks[0].mute = true;
    muted.output.audio = false;
    assert!(has(&muted, "MVP-AUDIO-AUTOMATION-TIME"));
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
            .any(|diagnostic| diagnostic.code == "MVP-LIMIT-AUDIO-GAIN-KEYFRAMES")
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
        assert!(has(&invalid, "MVP-AUDIO-ASSET"));
    }

    let mut output_disabled =
        project(json!({"tracks": [track("music", vec![clip("clip-a", "missing")])]}));
    output_disabled.output.audio = false;
    assert!(has(&output_disabled, "MVP-AUDIO-ASSET"));
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
        "MVP-LIMIT-AUDIO-TRACKS"
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
        "MVP-LIMIT-AUDIO-CLIPS"
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
            .filter(|code| *code == "MVP-AUDIO-EFFECT-ID")
            .count(),
        6
    );

    let allowed = project(json!({
        "tracks": [
            {"id": "track-a", "clips": [{"id": "clip-a", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "same", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}]}]},
            {"id": "track-b", "clips": [{"id": "clip-b", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "same", "type": "parametric_eq", "frequency_hz": 120, "gain_db": 1, "q": 1}]}]}
        ]
    }));
    assert!(!has(&allowed, "MVP-AUDIO-EFFECT-ID"));
}

#[test]
fn playback_speed_is_clip_only_and_uses_declared_bounds() {
    let valid = project(
        json!({"tracks": [{"id": "track", "clips": [{"id": "clip", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "speed", "type": "playback_speed", "rate": 0.25}]}]}]}),
    );
    assert!(!has(&valid, "MVP-AUDIO-EFFECT-SCOPE"));
    let invalid = project(
        json!({"effects": [{"id": "speed", "type": "playback_speed", "rate": 2}], "tracks": [{"id": "track", "effects": [{"id": "speed-track", "type": "playback_speed", "rate": 2}], "clips": []}]}),
    );
    assert_eq!(
        codes(&invalid)
            .iter()
            .filter(|code| *code == "MVP-AUDIO-EFFECT-SCOPE")
            .count(),
        2
    );
    for rate in [0.0, 0.249, 4.001] {
        let out_of_range = project(json!({
            "tracks": [{"id": "track", "clips": [{"id": "clip", "asset": "audio", "start": 0, "trim_start": 0, "effects": [{"id": "speed", "type": "playback_speed", "rate": rate}]}]}]
        }));
        assert!(has(&out_of_range, "MVP-AUDIO-EFFECT-PARAMETER"));
    }
}
