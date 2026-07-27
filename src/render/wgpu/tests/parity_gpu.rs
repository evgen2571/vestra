//! Adapter-dependent CPU/WGPU parity tests.

use std::sync::Arc;

use super::{
    compare_rgba,
    frame_plan::GpuFramePlan,
    gpu::wgpu_backend_or_skip,
    parameters::{FrameParameterArena, LayerParameters},
    requirements::GpuRequirements,
};
use crate::{
    animation::Track,
    domain::Point,
    plan::{
        ActiveSchedule, CompileOptions, CompiledEffect, CompiledSizing, CompiledVisualSource,
        EvaluatedEffect, EvaluatedFrame, RenderPlan, ScheduleAction, ScheduledItem, TimedEffect,
        compile,
    },
    project::{ValidationOptions, load_and_validate},
    render::{CpuBackend, RenderBackend, effects::effect_pass_plan},
};
use bytemuck::Zeroable;
use image::RgbaImage;

fn evaluated_effect_pass_count(frame: &EvaluatedFrame) -> usize {
    frame
        .layers
        .iter()
        .flat_map(|layer| &layer.effects)
        .chain(&frame.post_effects)
        .map(|effect| effect_pass_plan(effect).as_slice().len())
        .sum()
}

/// Prepares exactly the parameter capacity an evaluated catalogue case uses.
/// The fixture retains a compiled glow so every case has the reusable
/// auxiliary role available; only the declared pass count changes per case.
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
    plan
}

fn gpu_effect_case_matches_cpu(
    base_plan: &RenderPlan,
    decoded: &Arc<crate::render::DecodedAssets>,
    frame: &EvaluatedFrame,
    name: &str,
    tolerance: u8,
) -> bool {
    let plan = plan_for_evaluated_effect_case(base_plan, frame);
    let mut cpu = CpuBackend::new(&plan, Arc::clone(decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(decoded)) else {
        return false;
    };
    let mut cpu_output = RgbaImage::new(frame.width, frame.height);
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    cpu.render_frame(frame, &mut cpu_output)
        .expect("CPU effect frame renders");
    gpu.render_frame(frame, &mut gpu_output)
        .expect("GPU effect frame renders");
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), tolerance);
    assert!(
        difference.maximum_absolute_channel_error <= tolerance,
        "{name} parity exceeded tolerance {tolerance}: {difference:?}"
    );
    true
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

#[test]
fn evaluated_effect_chain_reserves_more_than_the_old_four_pass_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("RGBA parity fixture validates");
    let mut base = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    base.layers[0].effects = vec![TimedEffect {
        start: 0,
        end: u128::MAX,
        effect: CompiledEffect::Glow {
            threshold: Track::new(0.4),
            radius: Track::new(2.0),
            intensity: Track::new(0.8),
            colour: [255, 170, 60, 255],
        },
    }];
    let mut frame = crate::plan::evaluate(&base, &[ScheduledItem(0)], 0);
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

    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let requirements = GpuRequirements::from_plan(
        &plan,
        &decoded,
        std::mem::size_of::<LayerParameters>() as u32,
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
            .push(LayerParameters::zeroed())
            .expect("every planned parameter record fits the prepared capacity");
    }
}

#[test]
fn gpu_background_frame_matches_cpu_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let frame = EvaluatedFrame {
        time: 0,
        background: plan.canvas.background,
        width: plan.canvas.width,
        height: plan.canvas.height,
        layers: Vec::new(),
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    };
    let mut cpu_output = RgbaImage::new(frame.width, frame.height);
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    cpu.render_frame(&frame, &mut cpu_output)
        .expect("CPU frame renders");
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("GPU frame renders");
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 0);
    assert_eq!(
        difference.maximum_absolute_channel_error, 0,
        "GPU background must be exact: {difference:?}"
    );
}

#[test]
fn gpu_image_layer_matches_cpu_within_two_channels_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let image_layer = plan
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                crate::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");
    let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], 0);
    let mut cpu_output = RgbaImage::new(frame.width, frame.height);
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    cpu.render_frame(&frame, &mut cpu_output)
        .expect("CPU frame renders");
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("GPU frame renders");
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
    assert!(
        difference.maximum_absolute_channel_error <= 2,
        "GPU image parity exceeded tolerance: {difference:?}"
    );
}

#[test]
fn gpu_multilayer_frame_uses_nonzero_dynamic_offsets_without_validation_errors() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
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
    let frame = crate::plan::evaluate(
        &plan,
        &[ScheduledItem(*first), ScheduledItem(*second)],
        1_750_000_000,
    );
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    let mut cpu_output = RgbaImage::new(frame.width, frame.height);
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    cpu.render_frame(&frame, &mut cpu_output)
        .expect("CPU frame renders");
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("WGPU validation scopes accept the complete multi-offset frame");
    let execution = gpu.last_execution_metrics();
    assert_eq!(execution.command_encoders, 1);
    assert_eq!(execution.queue_submissions, 1);
    assert_eq!(execution.compute_passes, 5); // clear + two layers + two composites
    assert_eq!(execution.dispatches, 5);
    assert_eq!(execution.texture_copies, 1);
    assert_eq!(execution.parameter_uploads, 1);
    assert_eq!(execution.bind_groups_created, 0);
    assert_eq!(execution.bind_groups_recreated_for_parameter_growth, 0);
    assert_eq!(execution.bind_group_cache_hits, 5);
    assert_eq!(execution.bind_group_cache_misses, 0);
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
    assert!(
        difference.maximum_absolute_channel_error <= 2,
        "multi-offset GPU parity exceeded tolerance: {difference:?}"
    );
}

#[test]
fn gpu_rgba_fixture_matches_cpu_with_transparent_edges_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("RGBA parity fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0)], 0);
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    let mut cpu_output = RgbaImage::new(frame.width, frame.height);
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    cpu.render_frame(&frame, &mut cpu_output)
        .expect("CPU fixture renders");
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("WGPU fixture renders");
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
    assert!(
        difference.maximum_absolute_channel_error <= 2,
        "RGBA fixture parity exceeded tolerance: {difference:?}"
    );
}

#[test]
fn gpu_matches_cpu_for_every_blend_mode_with_overlapping_partial_alpha_rgba_layers() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("RGBA parity fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let mut upper = plan.layers[0].clone();
    upper.id = "overlapping-rgba-layer".to_owned();
    upper.opacity = Track::new(0.61);
    upper.transform.position = Track::new(Point { x: 0.56, y: 0.46 });
    plan.layers[0].opacity = Track::new(0.73);
    plan.layers.push(upper);
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let mut cpu = CpuBackend::new(&plan, decoded);
    for mode in [
        crate::project::BlendMode::Normal,
        crate::project::BlendMode::Add,
        crate::project::BlendMode::Screen,
        crate::project::BlendMode::Multiply,
        crate::project::BlendMode::Overlay,
    ] {
        let mut frame = crate::plan::evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 0);
        frame.layers[1].blend_mode = mode;
        assert!(frame.layers.iter().all(|layer| layer.opacity < 1.0));
        let mut cpu_output = RgbaImage::new(frame.width, frame.height);
        let mut gpu_output = RgbaImage::new(frame.width, frame.height);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU blend frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("GPU blend frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "{mode:?} blend parity exceeded tolerance: {difference:?}"
        );
    }
}

#[test]
fn gpu_matches_cpu_for_generated_preset_transition_camera_shake_and_flash_frames() {
    let cases = [
        (
            "heavy-impact preset and camera shake",
            "examples/presets/heavy-impact.json",
            1_550_000_000,
            4,
        ),
        (
            "flash-cut transition",
            "examples/transitions/flash-cut.json",
            2_500_000_000,
            2,
        ),
        (
            "flash overlay",
            "examples/projects/animation-effects.json",
            1_150_000_000,
            2,
        ),
    ];
    for (name, path, time, tolerance) in cases {
        let validated = load_and_validate(
            std::path::Path::new(path),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("generated feature fixture validates");
        let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        let active = active_items_at(&plan, time);
        let frame = crate::plan::evaluate(&plan, &active, time);
        match name {
            "heavy-impact preset and camera shake" => assert!(
                frame.layers[0]
                    .effects
                    .iter()
                    .any(|effect| matches!(effect, EvaluatedEffect::CameraShake { .. }))
            ),
            "flash-cut transition" => assert!(
                frame
                    .layers
                    .iter()
                    .flat_map(|layer| &layer.effects)
                    .any(|effect| matches!(effect, EvaluatedEffect::Tint { .. }))
            ),
            "flash overlay" => assert!(frame.layers.iter().any(|layer| matches!(
                layer.source,
                crate::plan::EvaluatedSource::SolidColor { .. }
            ))),
            _ => unreachable!("the test cases above are exhaustive"),
        }
        let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
        let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
            return;
        };
        let mut cpu_output = RgbaImage::new(frame.width, frame.height);
        let mut gpu_output = RgbaImage::new(frame.width, frame.height);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU generated frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("GPU generated frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), tolerance);
        assert!(
            difference.maximum_absolute_channel_error <= tolerance,
            "{name} parity exceeded tolerance {tolerance}: {difference:?}"
        );
    }
}

#[test]
fn gpu_composite_matches_cpu_for_sizing_transforms_effects_and_alpha() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let canonical = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&canonical).expect("fixture decodes");
    let image_layers = canonical
        .layers
        .iter()
        .enumerate()
        .filter_map(|(index, layer)| {
            matches!(layer.source, CompiledVisualSource::Image { .. }).then_some(index)
        })
        .collect::<Vec<_>>();
    let [red, blue] = image_layers.as_slice() else {
        panic!("canonical fixture must have two image layers");
    };

    for sizing in [
        CompiledSizing::Original,
        CompiledSizing::Fit,
        CompiledSizing::Cover,
        CompiledSizing::Scale(0.73),
        CompiledSizing::Stretch {
            width: 177,
            height: 91,
        },
    ] {
        let mut plan = canonical.clone();
        let CompiledVisualSource::Image {
            sizing: layer_sizing,
            ..
        } = &mut plan.layers[*red].source
        else {
            unreachable!()
        };
        *layer_sizing = sizing.clone();
        plan.layers[*red].transform.position = Track::new(Point { x: 0.47, y: 0.54 });
        plan.layers[*red].transform.anchor = Track::new(Point { x: 0.31, y: 0.67 });
        plan.layers[*red].transform.scale = Track::new(Point { x: 0.79, y: 1.13 });
        plan.layers[*red].transform.rotation_radians = Track::new(0.31);
        plan.layers[*red].opacity = Track::new(0.63);
        plan.layers[*red].effects = vec![
            CompiledEffect::Brightness {
                amount: Track::new(0.08),
            },
            CompiledEffect::Contrast {
                amount: Track::new(0.82),
            },
            CompiledEffect::Saturation {
                amount: Track::new(0.68),
            },
            CompiledEffect::Tint {
                colour: [28, 156, 231, 255],
                amount: Track::new(0.19),
            },
        ]
        .into_iter()
        .map(|effect| crate::plan::TimedEffect {
            start: 0,
            end: u128::MAX,
            effect,
        })
        .collect();
        let frame = crate::plan::evaluate(&plan, &[ScheduledItem(*red)], 750_000_000);
        let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
            return;
        };
        let mut cpu_output = RgbaImage::new(frame.width, frame.height);
        let mut gpu_output = RgbaImage::new(frame.width, frame.height);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("GPU frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "{sizing:?} parity exceeded tolerance: {difference:?}"
        );
    }

    let mut plan = canonical.clone();
    plan.layers[*red].opacity = Track::new(0.47);
    plan.layers[*blue].opacity = Track::new(0.58);
    let frame = crate::plan::evaluate(
        &plan,
        &[ScheduledItem(*red), ScheduledItem(*blue)],
        1_750_000_000,
    );
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let mut cpu_output = RgbaImage::new(frame.width, frame.height);
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    cpu.render_frame(&frame, &mut cpu_output)
        .expect("CPU frame renders");
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("GPU frame renders");
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
    assert!(
        difference.maximum_absolute_channel_error <= 2,
        "transparent multi-layer parity exceeded tolerance: {difference:?}"
    );
}

#[test]
fn gpu_canonical_timeline_frames_match_cpu_within_two_channels() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let schedule = ActiveSchedule::compile(&plan);
    let mut cursor = schedule.cursor();
    let mut active = Vec::new();
    for frame_index in 0..plan.frame_count {
        for event in cursor.events_at(frame_index) {
            match event.action {
                ScheduleAction::Activate => active.push(event.item),
                ScheduleAction::Deactivate => active.retain(|item| *item != event.item),
            }
        }
        active.sort_by(|left, right| {
            plan.layers[left.0]
                .draw_key
                .cmp(&plan.layers[right.0].draw_key)
        });
        if ![0, 14, 24, 28, 36, 42, 48, 59].contains(&frame_index) {
            continue;
        }
        let time =
            crate::timeline::frame_time_nanos(frame_index, plan.frame_rate.0, plan.frame_rate.1);
        let evaluated = crate::plan::evaluate(&plan, &active, time);
        let mut cpu_output = RgbaImage::new(evaluated.width, evaluated.height);
        let mut gpu_output = RgbaImage::new(evaluated.width, evaluated.height);
        cpu.render_frame(&evaluated, &mut cpu_output)
            .expect("CPU frame renders");
        gpu.render_frame(&evaluated, &mut gpu_output)
            .expect("GPU frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "canonical frame {frame_index} raw parity exceeded tolerance: {difference:?}"
        );
    }
}

#[test]
fn gpu_effect_catalogue_matches_cpu_on_the_rgba_fixture_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("RGBA parity fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    // Exercise odd output dimensions as well as the fixture's transparent,
    // partial-alpha, sharp-edge, and nonuniform-colour source pixels.
    plan.canvas.width = 173;
    plan.canvas.height = 129;
    // Preparation allocates every reusable working role the cases below use.
    plan.layers[0].effects = vec![TimedEffect {
        start: 0,
        end: u128::MAX,
        effect: CompiledEffect::Glow {
            threshold: Track::new(0.4),
            radius: Track::new(2.0),
            intensity: Track::new(0.8),
            colour: [255, 170, 60, 255],
        },
    }];
    plan.compilation.effect_pass_count = 4;
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
    let base = crate::plan::evaluate(&plan, &[ScheduledItem(0)], 0);
    let cases = [
        (
            "brightness",
            EvaluatedEffect::Brightness { amount: 0.12 },
            2,
        ),
        ("contrast", EvaluatedEffect::Contrast { amount: 1.18 }, 2),
        (
            "saturation",
            EvaluatedEffect::Saturation { amount: 0.63 },
            2,
        ),
        (
            "tint",
            EvaluatedEffect::Tint {
                colour: [20, 170, 255, 255],
                amount: 0.32,
            },
            2,
        ),
        (
            "gaussian",
            EvaluatedEffect::GaussianBlur { radius: 2.25 },
            4,
        ),
        (
            "glow",
            EvaluatedEffect::Glow {
                threshold: 0.4,
                radius: 2.25,
                intensity: 0.8,
                colour: [255, 170, 60, 255],
            },
            5,
        ),
        (
            "sharpen",
            EvaluatedEffect::Sharpen {
                amount: 0.65,
                radius: 2.25,
            },
            5,
        ),
        (
            "directional blur",
            EvaluatedEffect::DirectionalBlur {
                radius: 4.0,
                angle_degrees: 31.0,
            },
            4,
        ),
        (
            "zoom blur",
            EvaluatedEffect::ZoomBlur {
                radius: 8.0,
                samples: 9,
                anchor: Point { x: 0.37, y: 0.61 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            5,
        ),
        (
            "motion blur",
            EvaluatedEffect::MotionBlur {
                radius: 4.0,
                angle_degrees: 31.0,
                intensity: 1.0,
                shutter_angle: 180.0,
                max_radius: 8.0,
                samples: 9,
            },
            4,
        ),
        (
            "chromatic aberration",
            EvaluatedEffect::ChromaticAberration {
                amount: 2.0,
                angle_degrees: 20.0,
            },
            4,
        ),
        (
            "vignette",
            EvaluatedEffect::Vignette {
                amount: 0.7,
                radius: 0.45,
                softness: 0.25,
                colour: [10, 20, 50, 255],
            },
            2,
        ),
        (
            "color adjustment",
            EvaluatedEffect::ColorAdjust {
                exposure: 0.2,
                gamma: 1.3,
                black_point: 0.05,
                white_point: 0.92,
            },
            3,
        ),
    ];
    for (name, effect, tolerance) in cases {
        let mut frame = base.clone();
        frame.layers[0].effects = vec![effect];
        if !gpu_effect_case_matches_cpu(&plan, &decoded, &frame, name, tolerance) {
            return;
        }
    }

    let chains = [
        (
            "basic colour plus gaussian",
            vec![
                EvaluatedEffect::Brightness { amount: 0.12 },
                EvaluatedEffect::Contrast { amount: 1.18 },
                EvaluatedEffect::GaussianBlur { radius: 2.25 },
            ],
            vec![],
            crate::project::BlendMode::Normal,
            5,
        ),
        (
            "gaussian plus glow",
            vec![
                EvaluatedEffect::GaussianBlur { radius: 2.25 },
                EvaluatedEffect::Glow {
                    threshold: 0.4,
                    radius: 2.25,
                    intensity: 0.8,
                    colour: [255, 170, 60, 255],
                },
            ],
            vec![],
            crate::project::BlendMode::Normal,
            6,
        ),
        (
            "glow plus sharpen",
            vec![
                EvaluatedEffect::Glow {
                    threshold: 0.4,
                    radius: 2.25,
                    intensity: 0.8,
                    colour: [255, 170, 60, 255],
                },
                EvaluatedEffect::Sharpen {
                    amount: 0.65,
                    radius: 2.25,
                },
            ],
            vec![],
            crate::project::BlendMode::Overlay,
            7,
        ),
        (
            "sharpen plus glow and global sharpen",
            vec![
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
            ],
            vec![EvaluatedEffect::Sharpen {
                amount: 0.45,
                radius: 2.25,
            }],
            crate::project::BlendMode::Screen,
            7,
        ),
    ];
    for (name, effects, post_effects, blend_mode, tolerance) in chains {
        let mut frame = base.clone();
        frame.layers[0].effects = effects;
        frame.layers[0].blend_mode = blend_mode;
        frame.post_effects = post_effects;
        if name == "sharpen plus glow and global sharpen" {
            assert_eq!(evaluated_effect_pass_count(&frame), 10);
            assert!(
                evaluated_effect_pass_count(&frame) > 4,
                "the old four-pass allocation must not cover this chain"
            );
        }
        if !gpu_effect_case_matches_cpu(&plan, &decoded, &frame, name, tolerance) {
            return;
        }
    }
}
