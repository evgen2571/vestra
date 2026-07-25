//! Headless WGPU resource ownership for the render backend.

#![allow(
    clippy::result_large_err,
    reason = "WGPU preparation retains structured user-facing diagnostics"
)]

use std::{sync::Arc, time::Instant};

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, DecodedAssets, RenderBackend, RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings},
    },
};
use bytemuck::Zeroable;
use image::RgbaImage;

mod context;
mod diagnostics;
mod executor;
mod parameters;
mod parity;
mod pipeline;
mod readback;
mod requirements;
mod resources;
pub(crate) mod support;

#[cfg(test)]
#[path = "tests/parity.rs"]
mod parity_tests;
#[cfg(test)]
#[path = "tests/readback.rs"]
mod readback_tests;
#[cfg(test)]
#[path = "tests/requirements.rs"]
mod requirements_tests;
#[cfg(test)]
#[path = "tests/shader.rs"]
mod shader_tests;

use context::GpuContext;
use diagnostics::finish_error_scopes;
use executor::dispatch_layer;
use parameters::LayerParameters;
pub use parity::{FrameDifference, PixelMismatch, compare_rgba};
use pipeline::LayerPipeline;
use readback::read_frame;
use requirements::GpuRequirements;
use resources::{FrameResources, SourceResources};

/// A headless WGPU session. Its textures, output target, staging buffer and
/// source uploads persist for the complete render lifetime.
pub struct WgpuBackend {
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    _layer_shader: wgpu::ShaderModule,
    _layer_pipeline: wgpu::ComputePipeline,
    _layer_bind_group_layout: wgpu::BindGroupLayout,
    _layer_parameters: wgpu::Buffer,
    frame: FrameResources,
    sources: SourceResources,
    stats: PreparationStats,
    timings: PreparationTimings,
    adapter: AdapterMetadata,
}

impl WgpuBackend {
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Result<Self, Diagnostic> {
        let started = Instant::now();
        let requirements = GpuRequirements::from_plan(
            plan,
            &decoded,
            std::mem::size_of::<LayerParameters>() as u32,
        )?;
        let context = GpuContext::create(plan, requirements)?;
        let GpuContext {
            instance,
            adapter,
            device,
            queue,
            adapter_metadata,
            adapter_limits: limits,
            adapter_request,
            device_request,
        } = context;
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        device.push_error_scope(wgpu::ErrorFilter::Internal);
        let pipeline_creation_started = Instant::now();
        let row_bytes = requirements.row_bytes;
        let LayerPipeline {
            shader: layer_shader,
            compute: layer_pipeline,
            bindings: layer_bind_group_layout,
            parameters: layer_parameters,
        } = LayerPipeline::create(&device);
        let frame = FrameResources::create(
            &device,
            plan,
            row_bytes,
            requirements.padded_row_bytes,
            requirements.copy_bytes,
        );
        let pipeline_creation = pipeline_creation_started.elapsed();
        let upload_started = Instant::now();
        let sources = SourceResources::create(
            &device,
            &queue,
            plan,
            &decoded,
            &layer_bind_group_layout,
            &frame.accumulation,
            &layer_parameters,
            limits.max_texture_dimension_2d,
        )?;
        let mut stats = decoded.stats().clone();
        stats.source_texture_count = sources._textures.len();
        stats.source_texture_bytes = sources.uploaded_texture_bytes;
        stats.sampler_count = 0;
        stats.uploaded_texture_count = sources._textures.len();
        stats.uploaded_texture_bytes = sources.uploaded_texture_bytes;
        stats.readback_buffer_count = 1;
        stats.readback_buffer_bytes = requirements.copy_bytes;
        stats.shader_module_count = 1;
        stats.pipeline_count = 1;
        stats.output_texture_count = 1;
        stats.accumulation_buffer_count = 1;
        stats.bind_group_count = sources.bind_groups.len() + 1;
        let mut timings = decoded.timings();
        timings.gpu_adapter_request = adapter_request;
        timings.gpu_device_request = device_request;
        timings.gpu_pipeline_creation = pipeline_creation;
        timings.texture_upload = upload_started.elapsed();
        timings.gpu_initialization = started.elapsed();
        device.poll(wgpu::Maintain::Wait);
        finish_error_scopes(&device, "WGPU-RESOURCE-CREATION")?;
        Ok(Self {
            _instance: instance,
            _adapter: adapter,
            device,
            queue,
            _layer_shader: layer_shader,
            _layer_pipeline: layer_pipeline,
            _layer_bind_group_layout: layer_bind_group_layout,
            _layer_parameters: layer_parameters,
            frame,
            sources,
            stats,
            timings,
            adapter: adapter_metadata,
        })
    }
}

impl RenderBackend for WgpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Wgpu
    }

    fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        // Queue writes and submissions report validation/internal failures
        // asynchronously. Capture them for this frame so the engine can abort
        // FFmpeg and retain a structured primary failure instead of relying on
        // WGPU's uncaptured-error handler.
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.device.push_error_scope(wgpu::ErrorFilter::Internal);
        // Parameter updates and dispatch order are derived solely from the
        // evaluated frame. Each submission observes its matching uniform data.
        let clear = LayerParameters {
            header: [
                frame.width,
                frame.height,
                self.frame.padded_row_bytes / 4,
                0,
            ],
            solid_or_background: frame.background.map(f64::from).map(|value| value as f32),
            ..LayerParameters::zeroed()
        };
        dispatch_layer(
            &self.device,
            &self.queue,
            &self._layer_pipeline,
            &self._layer_parameters,
            &self.sources.solid_bind_group,
            clear,
            frame.width,
            frame.height,
        );
        for layer in &frame.layers {
            if let crate::plan::EvaluatedSource::Image {
                asset_index,
                crop,
                sizing,
                transform,
                cacheable_crop,
                ..
            } = &layer.source
            {
                let (source_width, source_height) = self.sources.dimensions[*asset_index];
                let parameters = parameters::image(
                    frame,
                    source_width,
                    source_height,
                    *crop,
                    *cacheable_crop,
                    sizing,
                    *transform,
                    layer.opacity,
                    layer.colour_transform,
                );
                dispatch_layer(
                    &self.device,
                    &self.queue,
                    &self._layer_pipeline,
                    &self._layer_parameters,
                    &self.sources.bind_groups[*asset_index],
                    parameters,
                    frame.width,
                    frame.height,
                );
            } else if let crate::plan::EvaluatedSource::SolidColor { colour } = layer.source {
                let parameters = LayerParameters {
                    header: [
                        frame.width,
                        frame.height,
                        self.frame.padded_row_bytes / 4,
                        2,
                    ],
                    effective: [0.0, 0.0, layer.opacity as f32, 0.0],
                    colour_row0: [
                        layer.colour_transform.matrix[0][0] as f32,
                        layer.colour_transform.matrix[0][1] as f32,
                        layer.colour_transform.matrix[0][2] as f32,
                        0.0,
                    ],
                    colour_row1: [
                        layer.colour_transform.matrix[1][0] as f32,
                        layer.colour_transform.matrix[1][1] as f32,
                        layer.colour_transform.matrix[1][2] as f32,
                        0.0,
                    ],
                    colour_row2: [
                        layer.colour_transform.matrix[2][0] as f32,
                        layer.colour_transform.matrix[2][1] as f32,
                        layer.colour_transform.matrix[2][2] as f32,
                        0.0,
                    ],
                    colour_offset: [
                        layer.colour_transform.offset[0] as f32,
                        layer.colour_transform.offset[1] as f32,
                        layer.colour_transform.offset[2] as f32,
                        0.0,
                    ],
                    solid_or_background: colour.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                };
                dispatch_layer(
                    &self.device,
                    &self.queue,
                    &self._layer_pipeline,
                    &self._layer_parameters,
                    &self.sources.solid_bind_group,
                    parameters,
                    frame.width,
                    frame.height,
                );
            }
        }
        let readback = read_frame(
            &self.device,
            &self.queue,
            &mut self.frame,
            frame.width,
            frame.height,
            destination,
        )?;
        self.timings.gpu_frame_command_encode += readback.command_encode;
        self.timings.gpu_submission += readback.submission;
        self.stats.command_submission_count += frame.layers.len() as u64 + 2;
        self.timings.gpu_readback_wait += readback.wait;
        self.timings.row_repack += readback.row_repack;
        Ok(())
    }

    fn stats(&mut self) -> PreparationStats {
        self.stats.clone()
    }

    fn timings(&self) -> PreparationTimings {
        self.timings
    }
    fn adapter(&self) -> Option<AdapterMetadata> {
        Some(self.adapter.clone())
    }
}

#[cfg(test)]
mod tests {
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
            let time = crate::timeline::frame_time_nanos(
                frame_index,
                plan.frame_rate.0,
                plan.frame_rate.1,
            );
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
            let frame =
                crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], time);
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
}
