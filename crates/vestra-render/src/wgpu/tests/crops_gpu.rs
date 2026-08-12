//! Adapter-dependent WGPU crop parity tests.

use std::sync::Arc;

use super::{compare_rgba, gpu::wgpu_backend_or_skip};
use crate::{
    animation::{Interpolation, Keyframe, Track},
    domain::Crop,
    plan::{CompileOptions, compile},
    project::{ValidationOptions, load_and_validate},
    render::CpuBackend,
};
use image::RgbaImage;

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
    let decoded = crate::DecodedAssets::build(&canonical).expect("fixture decodes");
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
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
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
