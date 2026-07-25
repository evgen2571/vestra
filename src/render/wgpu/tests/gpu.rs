
use super::{WgpuBackend, compare_rgba};
use crate::{
    animation::{Interpolation, Keyframe, Track},
    domain::{Crop, Point},
    plan::{
        ActiveSchedule, CompileOptions, CompiledEffect, CompiledSizing, CompiledVisualSource,
        EvaluatedFrame, ScheduleAction, ScheduledItem, compile,
    },
    project::{ValidationOptions, load_and_validate},
    render::{CpuBackend, RenderBackend},
};
use image::RgbaImage;
use std::sync::Arc;

fn wgpu_backend_or_skip(
    plan: &crate::plan::RenderPlan,
    decoded: Arc<crate::render::DecodedAssets>,
) -> Option<WgpuBackend> {
    match WgpuBackend::new(plan, decoded) {
        Ok(backend) => Some(backend),
        Err(error) if std::env::var_os("VIDEO_EDITOR_REQUIRE_WGPU").is_some() => {
            panic!(
                "strict WGPU verification requires an adapter and device: {}",
                error.message
            )
        }
        Err(error) => {
            eprintln!("skipping adapter-dependent WGPU test: {}", error.message);
            None
        }
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
fn gpu_readback_preserves_padded_rows_when_an_adapter_is_available() {
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
    let image_layer = canonical
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                crate::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");

    for width in [
        62, 64, 66, 126, 128, 130, 318, 320, 322, 718, 720, 722, 1080,
    ] {
        let mut plan = canonical.clone();
        plan.canvas.width = width;
        plan.canvas.height = 18;
        let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], 0);
        let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
            return;
        };
        let mut cpu_output = RgbaImage::new(width, frame.height);
        let mut gpu_output = RgbaImage::new(width, frame.height);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("GPU frame renders");
        assert_eq!(
            gpu_output.as_raw().len(),
            (width * frame.height * 4) as usize
        );
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "width {width} readback mismatch: {difference:?}"
        );
    }
}

#[test]
fn gpu_resources_are_reused_across_frames_when_an_adapter_is_available() {
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
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let initial = gpu.stats();
    assert_eq!(initial.shader_module_count, 1);
    assert_eq!(initial.pipeline_count, 1);
    assert_eq!(initial.uploaded_texture_count, plan.images.len());
    assert_eq!(initial.source_texture_count, plan.images.len());
    assert_eq!(initial.source_texture_bytes, initial.uploaded_texture_bytes);
    assert_eq!(initial.sampler_count, 0);
    assert_eq!(initial.output_texture_count, 1);
    assert_eq!(initial.accumulation_buffer_count, 1);
    assert_eq!(initial.readback_buffer_count, 1);

    for time in [0, 500_000_000, 1_000_000_000] {
        let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], time);
        let mut output = RgbaImage::new(frame.width, frame.height);
        gpu.render_frame(&frame, &mut output)
            .expect("GPU frame renders");
    }
    let final_stats = gpu.stats();
    assert_eq!(
        final_stats.uploaded_texture_count,
        initial.uploaded_texture_count
    );
    assert_eq!(
        final_stats.source_texture_count,
        initial.source_texture_count
    );
    assert_eq!(
        final_stats.source_texture_bytes,
        initial.source_texture_bytes
    );
    assert_eq!(final_stats.shader_module_count, initial.shader_module_count);
    assert_eq!(final_stats.pipeline_count, initial.pipeline_count);
    assert_eq!(
        final_stats.output_texture_count,
        initial.output_texture_count
    );
    assert_eq!(
        final_stats.accumulation_buffer_count,
        initial.accumulation_buffer_count
    );
    assert_eq!(
        final_stats.readback_buffer_count,
        initial.readback_buffer_count
    );
    assert_eq!(final_stats.command_submission_count, 9);
}

#[test]
fn gpu_static_crops_match_cpu_when_an_adapter_is_available() {
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
    let image_layer = canonical
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                crate::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");

    for crop in [
        Crop {
            x: 0.13,
            y: 0.17,
            width: 0.61,
            height: 0.59,
        },
        Crop {
            x: 0.0,
            y: 0.11,
            width: 0.71,
            height: 0.73,
        },
        Crop {
            x: 0.29,
            y: 0.0,
            width: 0.71,
            height: 0.73,
        },
        Crop {
            x: 0.29,
            y: 0.27,
            width: 0.71,
            height: 0.73,
        },
        Crop {
            x: 0.13,
            y: 0.27,
            width: 0.61,
            height: 0.73,
        },
    ] {
        let mut plan = canonical.clone();
        let crate::plan::CompiledVisualSource::Image {
            crop: track,
            cacheable_crop,
            ..
        } = &mut plan.layers[image_layer].source
        else {
            unreachable!()
        };
        *track = Track::new(crop);
        *cacheable_crop = true;
        let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], 0);
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
            "static crop {crop:?}: {difference:?}"
        );
    }
}

#[test]
fn gpu_animated_crop_matches_cpu_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("canonical fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::render::DecodedAssets::build(&plan).expect("fixture decodes");
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
    let crate::plan::CompiledVisualSource::Image {
        crop,
        cacheable_crop,
        ..
    } = &mut plan.layers[image_layer].source
    else {
        unreachable!()
    };
    *crop = Track {
        base_value: Crop {
            x: 0.08,
            y: 0.14,
            width: 0.78,
            height: 0.72,
        },
        keyframes: vec![Keyframe {
            time: 1_000_000_000,
            value: Crop {
                x: 0.19,
                y: 0.21,
                width: 0.67,
                height: 0.63,
            },
            interpolation: Interpolation::Linear,
        }],
    };
    *cacheable_crop = false;
    let frame = crate::plan::evaluate(
        &plan,
        &[crate::plan::ScheduledItem(image_layer)],
        500_000_000,
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
        "animated crop: {difference:?}"
    );
}
