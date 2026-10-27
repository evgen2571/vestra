//! WGPU-private planning and execution assertions split from cross-backend parity.
use super::{
    frame_plan::GpuFramePlan,
    gpu::wgpu_backend_or_skip,
    parameters::{FrameParameterArena, LayerParameters},
    requirements::GpuRequirements,
};
use crate::{
    animation::Track,
    test_support::{ValidationOptions, load_and_validate},
};
use bytemuck::Zeroable;
use image::RgbaImage;
use serde_json::{Value, json};
use vestra_core::plan::{
    ColourTransform, CompileOptions, CompiledEffect, CompiledScalarProperty, CompiledVisualSource,
    EvaluatedEffect, EvaluatedFrame, RenderPlan, ScheduledItem, TimedEffect, compile,
    effect_pass_plan,
};
#[path = "../../../../../tests/render_effect_cases.rs"]
mod effect_cases;
use effect_cases::{
    catalogue_cases, evaluated_effect_pass_count, spectrum_frame, spectrum_style_cases,
};

fn scalar(track: Track<f64>) -> CompiledScalarProperty {
    CompiledScalarProperty::authored(track)
}

fn group_project(
    children: Vec<Value>,
    group: Value,
    transitions: Vec<Value>,
) -> crate::project::Project {
    let mut value = json!({
        "schema_version": 1,
        "output": {
            "path": "group-parity.mp4", "width": 32, "height": 32,
            "frame_rate": "24/1", "background": "#101018", "quality": "preview",
            "audio": false, "duration_mode": "explicit", "duration": 2.0
        },
        "assets": [],
        "visual": {
            "clips": [{
                "id": "group", "source": {"type": "group", "clips": children},
                "start": 0.0, "duration": 2.0, "layer": 0,
                "opacity": {"base_value": 1.0}
            }],
            "transitions": transitions, "flashes": [], "post_effects": []
        }
    });
    if let Some(object) = value["visual"]["clips"][0].as_object_mut()
        && let Some(group_object) = group.as_object()
    {
        for (key, value) in group_object {
            object.insert(key.clone(), value.clone());
        }
    }
    serde_json::from_value(value.take()).expect("Group parity project parses")
}

fn active_items_at(plan: &RenderPlan, time: u128) -> Vec<ScheduledItem> {
    let mut active = plan
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| {
            (layer.start_nanos <= time && time < layer.start_nanos + layer.duration_nanos)
                .then_some(ScheduledItem(index))
        })
        .collect::<Vec<_>>();
    active.sort_by(|left, right| {
        plan.layers[left.0]
            .draw_key
            .cmp(&plan.layers[right.0].draw_key)
    });
    active
}

fn assert_particle_group_plan_targets_group_canvas(project: &crate::project::Project) {
    let report = vestra_core::validation::validate(
        project,
        vestra_core::validation::ResourceLimits::default(),
    );
    assert!(report.is_valid(), "particle Group project is invalid");
    let assets = std::collections::BTreeMap::new();
    let durations = std::collections::BTreeMap::new();
    let input = vestra_core::plan::PlanCompileInput::new(
        project,
        vestra_core::validation::ResourceLimits::default(),
        std::path::Path::new("."),
        &assets,
        &durations,
        1.0,
        (24, 1),
        24,
        &[],
    );
    let plan = compile(input, CompileOptions::default()).expect("particle Group plan compiles");
    let frame = vestra_core::plan::evaluate(&plan, &active_items_at(&plan, 0), 0)
        .expect("renderer fixture evaluates");
    let frame_plan = GpuFramePlan::build(&frame);
    frame_plan
        .validate(0)
        .expect("particle Group plan validates");
    assert!(frame_plan.operations.iter().any(|operation| matches!(
        operation,
        super::frame_plan::GpuOperation::ResolveParticleLayer { .. }
    )));
    assert!(frame_plan.operations.iter().any(|operation| matches!(
        operation,
        super::frame_plan::GpuOperation::CompositeLayer {
            canvas_destination: super::frame_plan::TextureSlot::GroupCanvasA(0)
                | super::frame_plan::TextureSlot::GroupCanvasB(0),
            ..
        }
    )));
}

fn plan_for_evaluated_effect_case(base: &RenderPlan, frame: &EvaluatedFrame) -> RenderPlan {
    let mut plan = base.clone();
    let effect_pass_count = evaluated_effect_pass_count(frame);
    let frame_plan = GpuFramePlan::build(frame);
    assert_eq!(
        frame_plan.parameter_count,
        (frame.layers.len() as u32 * 2) + effect_pass_count as u32 + 1,
        "evaluated frame parameter count must match the prepared capacity formula"
    );
    plan.compilation.effect_pass_count = effect_pass_count;
    if frame
        .layers
        .iter()
        .flat_map(|layer| &layer.effects)
        .chain(&frame.post_effects)
        .any(|effect| effect_pass_plan(effect).requirements().retains_original())
        && !super::frame_plan::plan_requires_auxiliary(&plan)
    {
        plan.post_effects.push(TimedEffect {
            start: 0,
            end: u128::MAX,
            effect: CompiledEffect::Bloom {
                threshold: scalar(Track::new(0.0)),
                radius: scalar(Track::new(1.0)),
                intensity: scalar(Track::new(1.0)),
            },
            dependency: vestra_core::plan::TemporalDependency::Static,
        });
    }
    plan
}

#[test]
fn evaluated_effect_chain_reserves_more_than_the_old_four_pass_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("RGBA parity fixture validates");
    let mut base = compile(validated, CompileOptions::default()).expect("fixture compiles");
    base.layers[0].effects = vec![TimedEffect {
        start: 0,
        end: u128::MAX,
        effect: CompiledEffect::Glow {
            threshold: scalar(Track::new(0.4)),
            radius: scalar(Track::new(2.0)),
            intensity: scalar(Track::new(0.8)),
            colour: [255, 170, 60, 255],
        },
        dependency: vestra_core::plan::TemporalDependency::Static,
    }];
    let mut frame = vestra_core::plan::evaluate(&base, &[ScheduledItem(0)], 0)
        .expect("renderer fixture evaluates");
    frame.layers[0].effects = vec![
        EvaluatedEffect::Sharpen {
            amount: 0.65,
            radius: 2.25,
        },
        EvaluatedEffect::Glow {
            threshold: 0.4,
            radius: 2.25,
            intensity: 0.8,
            colour: [255, 170, 60, 255],
        },
    ];
    frame.post_effects = vec![EvaluatedEffect::Sharpen {
        amount: 0.45,
        radius: 2.25,
    }];
    let plan = plan_for_evaluated_effect_case(&base, &frame);
    let frame_plan = GpuFramePlan::build(&frame);
    assert_eq!(evaluated_effect_pass_count(&frame), 10);
    assert_eq!(frame_plan.parameter_count, 13);
    assert!(plan.compilation.effect_pass_count > 4);

    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        super::parameters::PARAMETER_RECORD_BYTES as u32,
    )
    .expect("requirements calculate");
    let mut arena = FrameParameterArena::new(
        256,
        requirements
            .parameter_buffer_bytes(256)
            .expect("prepared parameter capacity calculates"),
    );
    for _ in 0..frame_plan.parameter_count {
        arena
            .push(&LayerParameters::zeroed())
            .expect("every planned parameter record fits the prepared capacity");
    }
    assert_eq!(
        arena
            .push(&LayerParameters::zeroed())
            .expect_err("one undeclared pass must exceed the prepared capacity")
            .code,
        "WGPU-PARAMETER-OVERFLOW"
    );
}

#[test]
fn generated_camera_shake_changes_geometry_without_creating_a_pixel_effect_pass() {
    let validated = load_and_validate(
        std::path::Path::new("examples/presets/heavy-impact.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("heavy-impact fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let before = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], 1_450_000_000)
        .expect("renderer fixture evaluates");
    let during = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], 1_550_000_000)
        .expect("renderer fixture evaluates");
    let before_transform = before.layers[0].transform;
    let vestra_core::plan::EvaluatedSource::Image { .. } = &before.layers[0].source else {
        unreachable!("heavy-impact clip uses an image")
    };
    let during_transform = during.layers[0].transform;
    let vestra_core::plan::EvaluatedSource::Image { .. } = &during.layers[0].source else {
        unreachable!("heavy-impact clip uses an image")
    };
    assert_ne!(before_transform.position, during_transform.position);
    let camera_shake = during.layers[0]
        .effects
        .iter()
        .find(|effect| matches!(effect, EvaluatedEffect::CameraShake { .. }))
        .expect("heavy impact generates camera shake");
    assert!(effect_pass_plan(camera_shake).is_empty());
    let planned_pixel_passes = GpuFramePlan::build(&during)
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation,
                super::frame_plan::GpuOperation::ApplyEffect { .. }
            )
        })
        .count();
    let expected_pixel_passes = during.layers[0]
        .effects
        .iter()
        .filter(|effect| !matches!(effect, EvaluatedEffect::CameraShake { .. }))
        .map(|effect| effect_pass_plan(effect).as_slice().len())
        .sum::<usize>();
    assert_eq!(planned_pixel_passes, expected_pixel_passes);
}

#[test]
fn gpu_particle_system_inside_group_targets_group_canvas() {
    let particle = json!({
        "id": "particles",
        "source": {
            "type": "particle_system", "seed": 41,
            "emitter": {"type": "point", "position": {"x": 0.5, "y": 0.5}},
            "emission": {"bursts": [{"time": 0.0, "count": 8}]},
            "particle": {
                "lifetime": 1.0, "size": 0.12, "speed": 0.0,
                "direction_spread_degrees": 0.0, "colour": "#FFD27A80",
                "primitive": "square", "blend_mode": "normal"
            }
        },
        "start": 0.0, "duration": 2.0, "layer": 0,
        "opacity": {"base_value": 1.0}
    });
    let mut additive = particle.clone();
    additive["id"] = json!("additive-particles");
    additive["source"]["particle"]["blend_mode"] = json!("additive");
    let project = group_project(vec![particle, additive], json!({}), vec![]);
    assert_particle_group_plan_targets_group_canvas(&project);
}

#[test]
fn gpu_multilayer_frame_preserves_execution_metrics() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let image_layers = plan
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| {
            matches!(layer.source, CompiledVisualSource::Image { .. }).then_some(index)
        })
        .collect::<Vec<_>>();
    let [first, second] = image_layers.as_slice() else {
        panic!("canonical fixture must contain two visible image layers");
    };
    let frame = vestra_core::plan::evaluate(
        &plan,
        &[ScheduledItem(*first), ScheduledItem(*second)],
        1_750_000_000,
    )
    .expect("renderer fixture evaluates");
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("WGPU validation scopes accept the complete multi-offset frame");
    let execution = gpu.last_execution_metrics();
    assert_eq!(execution.command_encoders, 1);
    assert_eq!(execution.queue_submissions, 1);
    assert_eq!(execution.compute_passes, 4);
    assert_eq!(execution.dispatches, 4);
    assert_eq!(execution.texture_copies, 1);
    assert_eq!(execution.parameter_uploads, 1);
    assert_eq!(execution.bind_groups_created, 0);
    assert_eq!(execution.bind_groups_recreated_for_parameter_growth, 0);
    assert_eq!(execution.bind_group_cache_hits, 4);
    assert_eq!(execution.bind_group_cache_misses, 0);
}

#[test]
fn gpu_effect_catalogue_parameter_counts_match_prepared_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("RGBA parity fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    // Exercise odd output dimensions as well as the fixture's transparent,
    // partial-alpha, sharp-edge, and nonuniform-colour source pixels.
    plan.canvas.width = 173;
    plan.canvas.height = 129;
    // Preparation allocates every reusable working role the cases below use.
    plan.layers[0].effects = vec![TimedEffect {
        start: 0,
        end: u128::MAX,
        effect: CompiledEffect::Glow {
            threshold: scalar(Track::new(0.4)),
            radius: scalar(Track::new(2.0)),
            intensity: scalar(Track::new(0.8)),
            colour: [255, 170, 60, 255],
        },
        dependency: vestra_core::plan::TemporalDependency::Static,
    }];
    plan.compilation.effect_pass_count = 4;
    let base = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], 0)
        .expect("renderer fixture evaluates");
    catalogue_cases(&base, |frame, _, _| {
        let mut frame = frame.clone();
        for layer in &mut frame.layers {
            layer.colour_transform = ColourTransform::from_effects(layer.effects.clone());
        }
        let _ = plan_for_evaluated_effect_case(&plan, &frame);
        true
    });
}

#[test]
fn gpu_spectrum2d_layout_and_style_parameter_counts_match_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 32;
    plan.canvas.height = 32;
    plan.compilation.effect_pass_count = 0;
    spectrum_style_cases(|frame, _, _| {
        let _ = plan_for_evaluated_effect_case(&plan, frame);
        true
    });
}

#[test]
fn gpu_spectrum2d_brightness_parameter_count_matches_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 10;
    plan.canvas.height = 4;
    plan.compilation.effect_pass_count = 0;
    let mut frame = spectrum_frame(vec![1.0, 0.0, 0.0], 0.0);
    let brightness = EvaluatedEffect::Brightness { amount: 0.12 };
    frame.layers[0].effects = vec![brightness.clone()];
    frame.layers[0].colour_transform = ColourTransform::from_effects([brightness]);

    let _ = plan_for_evaluated_effect_case(&plan, &frame);
}

#[test]
fn gpu_spectrum2d_brightness_then_bloom_parameter_count_matches_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 10;
    plan.canvas.height = 4;
    plan.compilation.effect_pass_count = 0;
    let mut frame = spectrum_frame(vec![1.0, 0.0, 0.0], 0.0);
    let brightness = EvaluatedEffect::Brightness { amount: 0.12 };
    frame.layers[0].effects = vec![
        brightness.clone(),
        EvaluatedEffect::Bloom {
            threshold: 0.1,
            radius: 2.0,
            intensity: 1.0,
        },
    ];
    frame.layers[0].colour_transform = ColourTransform::from_effects([brightness]);

    let _ = plan_for_evaluated_effect_case(&plan, &frame);
}

#[test]
fn gpu_spectrum2d_bloom_parameter_count_matches_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 10;
    plan.canvas.height = 4;
    plan.compilation.effect_pass_count = 0;
    let mut frame = spectrum_frame(vec![1.0, 0.0, 0.0], 0.0);
    frame.layers[0].effects = vec![EvaluatedEffect::Bloom {
        threshold: 0.1,
        radius: 2.0,
        intensity: 1.0,
    }];
    let _ = plan_for_evaluated_effect_case(&plan, &frame);
}
