//! Cross-backend padded-row regression.
use super::{compare_rgba, gpu::wgpu_backend_or_skip, lifecycle::RenderFrame};
use crate::test_support::{ValidationOptions, load_and_validate};
use image::RgbaImage;
use std::sync::Arc;
use vestra_core::plan::{CompileOptions, compile};
use vestra_render::CpuBackend;

#[test]
fn gpu_readback_preserves_padded_rows_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("canonical fixture validates");
    let canonical = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&canonical).expect("fixture decodes");
    let image_layer = canonical
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                vestra_core::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");

    for width in [
        62, 64, 66, 126, 128, 130, 318, 320, 322, 718, 720, 722, 1080,
    ] {
        let mut plan = canonical.clone();
        plan.canvas.width = width;
        plan.canvas.height = 18;
        let frame =
            vestra_core::plan::evaluate(&plan, &[vestra_core::plan::ScheduledItem(image_layer)], 0)
                .expect("renderer fixture evaluates");
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
