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
    render::{CpuBackend, effects::effect_pass_plan},
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
    decoded: &Arc<crate::DecodedAssets>,
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

    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
    assert_eq!(
        arena
            .push(LayerParameters::zeroed())
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
            ..ValidationOptions::default()
        },
    )
    .expect("heavy-impact fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let before = crate::plan::evaluate(&plan, &[ScheduledItem(0)], 1_450_000_000);
    let during = crate::plan::evaluate(&plan, &[ScheduledItem(0)], 1_550_000_000);
    let crate::plan::EvaluatedSource::Image {
        transform: before_transform,
        ..
    } = &before.layers[0].source
    else {
        unreachable!("heavy-impact clip uses an image")
    };
    let crate::plan::EvaluatedSource::Image {
        transform: during_transform,
        ..
    } = &during.layers[0].source
    else {
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
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
fn gpu_matches_cpu_for_every_blend_mode_and_alpha_case_on_the_rgba_fixture() {
    let validated = load_and_validate(
        std::path::Path::new("tests/fixtures/wgpu-small-rgba.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("RGBA parity fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 173;
    plan.canvas.height = 129;
    let mut upper = plan.layers[0].clone();
    upper.id = "overlapping-rgba-layer".to_owned();
    upper.opacity = Track::new(1.0);
    upper.transform.position = Track::new(Point { x: 0.56, y: 0.46 });
    upper.effects = vec![TimedEffect {
        start: 0,
        end: u128::MAX,
        effect: CompiledEffect::Vignette {
            amount: Track::new(0.25),
            radius: Track::new(0.55),
            softness: Track::new(0.2),
            colour: [0, 255, 1, 255],
        },
    }];
    plan.layers[0].opacity = Track::new(1.0);
    plan.layers.push(upper);
    plan.compilation.effect_pass_count = 1;
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let mut cpu = CpuBackend::new(&plan, decoded);
    let cases = [
        ("opaque layers", [26, 26, 40, 255], 1.0, 1.0),
        ("transparent source", [26, 26, 40, 255], 1.0, 0.0),
        ("transparent destination", [26, 26, 40, 0], 0.0, 1.0),
        ("partial source alpha", [26, 26, 40, 255], 1.0, 0.61),
        ("partial destination alpha", [26, 26, 40, 96], 0.47, 1.0),
        (
            "partial source and destination",
            [26, 26, 40, 128],
            0.73,
            0.61,
        ),
        ("extreme channels", [0, 255, 1, 255], 1.0, 0.61),
    ];
    assert_eq!(crate::project::BlendMode::ALL.len(), 5);
    for mode in crate::project::BlendMode::ALL {
        for (case, background, lower_opacity, upper_opacity) in cases {
            let mut frame = crate::plan::evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 0);
            frame.background = background;
            frame.layers[0].opacity = lower_opacity;
            frame.layers[1].opacity = upper_opacity;
            frame.layers[1].blend_mode = mode;
            let mut cpu_output = RgbaImage::new(frame.width, frame.height);
            let mut gpu_output = RgbaImage::new(frame.width, frame.height);
            cpu.render_frame(&frame, &mut cpu_output)
                .expect("CPU blend frame renders");
            gpu.render_frame(&frame, &mut gpu_output)
                .expect("GPU blend frame renders");
            let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
            assert!(
                difference.maximum_absolute_channel_error <= 2,
                "{mode:?} {case} parity exceeded tolerance: max={} mean={} differing_pixels={} worst={:?}",
                difference.maximum_absolute_channel_error,
                difference.mean_absolute_channel_error,
                difference.pixels_exceeding_tolerance,
                difference.first_significant_mismatch,
            );
        }
    }
}

#[test]
fn gpu_matches_cpu_for_generated_preset_transition_camera_shake_and_flash_frames() {
    let cases = [
        (
            "impact preset",
            "examples/presets/impact.json",
            1_535_000_000,
            4,
        ),
        (
            "heavy-impact preset and camera shake",
            "examples/presets/heavy-impact.json",
            1_550_000_000,
            4,
        ),
        (
            "focus-reveal preset",
            "examples/presets/focus-reveal.json",
            1_600_000_000,
            4,
        ),
        (
            "zoom-blur transition before",
            "examples/transitions/zoom-blur.json",
            1_999_000_000,
            2,
        ),
        (
            "zoom-blur transition start",
            "examples/transitions/zoom-blur.json",
            2_000_000_000,
            2,
        ),
        (
            "zoom-blur transition midpoint",
            "examples/transitions/zoom-blur.json",
            2_500_000_000,
            5,
        ),
        (
            "zoom-blur transition end",
            "examples/transitions/zoom-blur.json",
            3_000_000_000,
            2,
        ),
        (
            "zoom-blur transition after",
            "examples/transitions/zoom-blur.json",
            3_001_000_000,
            2,
        ),
        (
            "flash-cut transition before",
            "examples/transitions/flash-cut.json",
            1_999_000_000,
            2,
        ),
        (
            "flash-cut transition start",
            "examples/transitions/flash-cut.json",
            2_000_000_000,
            2,
        ),
        (
            "flash-cut transition",
            "examples/transitions/flash-cut.json",
            2_500_000_000,
            2,
        ),
        (
            "flash-cut transition end",
            "examples/transitions/flash-cut.json",
            3_000_000_000,
            2,
        ),
        (
            "flash-cut transition after",
            "examples/transitions/flash-cut.json",
            3_001_000_000,
            2,
        ),
        (
            "flash overlay before",
            "examples/projects/animation-effects.json",
            1_099_000_000,
            2,
        ),
        (
            "flash overlay",
            "examples/projects/animation-effects.json",
            1_150_000_000,
            2,
        ),
        (
            "flash overlay end",
            "examples/projects/animation-effects.json",
            1_250_000_000,
            2,
        ),
        (
            "flash overlay after",
            "examples/projects/animation-effects.json",
            1_251_000_000,
            2,
        ),
    ];
    let mut adapter_available = true;
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
            "impact preset" => assert!(matches!(
                frame.layers[0].effects.as_slice(),
                [
                    EvaluatedEffect::CameraShake { .. },
                    EvaluatedEffect::ChromaticAberration { .. },
                    EvaluatedEffect::Tint { .. },
                ]
            )),
            "heavy-impact preset and camera shake" => assert!(matches!(
                frame.layers[0].effects.as_slice(),
                [
                    EvaluatedEffect::CameraShake { .. },
                    EvaluatedEffect::DirectionalBlur { .. },
                    EvaluatedEffect::ChromaticAberration { .. },
                    EvaluatedEffect::Tint { .. },
                ]
            )),
            "focus-reveal preset" => assert!(matches!(
                frame.layers[0].effects.as_slice(),
                [
                    EvaluatedEffect::GaussianBlur { .. },
                    EvaluatedEffect::Sharpen { .. }
                ]
            )),
            "zoom-blur transition midpoint" => assert!(
                frame
                    .layers
                    .iter()
                    .flat_map(|layer| &layer.effects)
                    .any(|effect| matches!(effect, EvaluatedEffect::ZoomBlur { .. }))
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
            _ => {}
        }
        if !adapter_available {
            continue;
        }
        let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
        let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
            adapter_available = false;
            continue;
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
fn gpu_flash_matches_cpu_for_opaque_and_global_post_effect_variants() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("flash fixture validates");
    let mut adapter_available = true;
    for (name, opaque_flash, global_post) in [
        ("opaque flash", true, false),
        ("partial flash with global post", false, true),
    ] {
        let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
        if global_post {
            plan.post_effects.push(TimedEffect {
                start: 0,
                end: u128::MAX,
                effect: CompiledEffect::Vignette {
                    amount: Track::new(0.18),
                    radius: Track::new(0.72),
                    softness: Track::new(0.35),
                    colour: [0, 0, 0, 255],
                },
            });
            plan.compilation.effect_pass_count += 1;
        }
        let time = 1_150_000_000;
        let active = active_items_at(&plan, time);
        let mut frame = crate::plan::evaluate(&plan, &active, time);
        let flash = frame
            .layers
            .iter_mut()
            .find(|layer| {
                matches!(
                    layer.source,
                    crate::plan::EvaluatedSource::SolidColor { .. }
                )
            })
            .expect("fixture has an active flash layer");
        if opaque_flash {
            flash.opacity = 1.0;
        }
        if !adapter_available {
            continue;
        }
        let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
        let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
            adapter_available = false;
            continue;
        };
        let mut cpu_output = RgbaImage::new(frame.width, frame.height);
        let mut gpu_output = RgbaImage::new(frame.width, frame.height);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU flash frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("GPU flash frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "{name} parity exceeded tolerance: {difference:?}"
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
    let decoded = crate::DecodedAssets::build(&canonical).expect("fixture decodes");
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
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
            "small directional blur",
            EvaluatedEffect::DirectionalBlur {
                radius: 0.12,
                angle_degrees: 31.0,
            },
            2,
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
            "small zoom blur",
            EvaluatedEffect::ZoomBlur {
                radius: 0.12,
                samples: 9,
                anchor: Point { x: 0.37, y: 0.61 },
                direction: crate::project::ZoomBlurDirection::Centered,
            },
            2,
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
            "small motion blur",
            EvaluatedEffect::MotionBlur {
                radius: 0.12,
                angle_degrees: 31.0,
                intensity: 1.0,
                shutter_angle: 180.0,
                max_radius: 8.0,
                samples: 9,
            },
            2,
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
