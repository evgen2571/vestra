//! Prepared WGPU texture-frame backend.

use std::{sync::Arc, time::Instant};

use bytemuck::Zeroable;
#[cfg(test)]
use image::RgbaImage;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, EvaluatedSource, RenderPlan},
    render::{
        AdapterMetadata, CompletedFrame, DecodedAssets, PollMode, RenderBackend, RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings, StagedMetrics},
    },
};

use super::{
    context::GpuContext,
    diagnostics::finish_error_scopes,
    executor::{FrameBindGroups, FrameExecutionMetrics, encode_and_submit},
    frame_plan::{GpuFramePlan, GpuOperation},
    parameters::{self, FrameParameterArena, LayerParameters},
    pipeline::GpuPipelines,
    polling::{drain, nonblocking, wait_for_one},
    readback::ReadbackRing,
    requirements::{GpuRequirements, ResourceEstimates},
    resources::{FrameResources, SourceResources},
};
use crate::{
    project::{BlendMode, ZoomBlurDirection},
    render::effects::EffectPass,
};

/// WGPU owns persistent source and working textures. Each submitted frame owns
/// its parameter buffer, bind groups, and readback slot until mapping completes.
pub struct WgpuBackend {
    context: GpuContext,
    pipelines: GpuPipelines,
    frame: FrameResources,
    sources: SourceResources,
    slots: Vec<FrameSlotResources>,
    readback: ReadbackRing,
    pipeline_depth: usize,
    resource_estimates: ResourceEstimates,
    last_execution: FrameExecutionMetrics,
    stats: PreparationStats,
    timings: PreparationTimings,
    staged: StagedMetrics,
    aborted: bool,
}

struct FrameSlotResources {
    parameters: FrameParameterArena,
    parameter_buffer: wgpu::Buffer,
    bind_groups: FrameBindGroups,
    uses: u64,
}

impl WgpuBackend {
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Result<Self, Diagnostic> {
        let depth = pipeline_depth_from_environment()?;
        Self::new_with_pipeline_depth(plan, decoded, depth)
    }

    pub fn new_with_pipeline_depth(
        plan: &RenderPlan,
        decoded: Arc<DecodedAssets>,
        pipeline_depth: usize,
    ) -> Result<Self, Diagnostic> {
        validate_pipeline_depth(pipeline_depth)?;
        let started = Instant::now();
        let requirements = GpuRequirements::from_plan(
            plan,
            &decoded,
            std::mem::size_of::<LayerParameters>() as u32,
        )?;
        let context = GpuContext::create(plan, requirements)?;
        let alignment = context.device.limits().min_uniform_buffer_offset_alignment;
        let parameter_buffer_bytes = requirements.parameter_buffer_bytes(alignment)?;
        let resource_estimates =
            requirements.resource_estimates_for_depth(alignment, pipeline_depth)?;
        let pipeline_started = Instant::now();
        let pipelines = GpuPipelines::create(&context.device);
        let frame = FrameResources::create(&context.device, plan, requirements.padded_row_bytes);
        debug_assert_eq!(
            frame.working.estimated_bytes(),
            resource_estimates.working_texture_bytes
        );
        debug_assert_eq!(
            frame.working.texture_count() as u64,
            resource_estimates.working_texture_count
        );
        let pipeline_creation = pipeline_started.elapsed();
        let upload_started = Instant::now();
        let sources = SourceResources::create(
            &context.device,
            &context.queue,
            plan,
            &decoded,
            context.adapter_limits.max_texture_dimension_2d,
        )?;
        let mut slots = Vec::with_capacity(pipeline_depth);
        let per_slot_bind_group_count = {
            let parameter_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("video-editor frame parameters"),
                size: parameter_buffer_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_groups = FrameBindGroups::create(
                &context.device,
                &pipelines,
                &frame,
                &sources,
                &parameter_buffer,
            );
            let count = bind_groups.persistent_created();
            slots.push(FrameSlotResources {
                parameters: FrameParameterArena::new(alignment, parameter_buffer_bytes),
                parameter_buffer,
                bind_groups,
                uses: 0,
            });
            count
        };
        while slots.len() < pipeline_depth {
            let parameter_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("video-editor frame parameters"),
                size: parameter_buffer_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_groups = FrameBindGroups::create(
                &context.device,
                &pipelines,
                &frame,
                &sources,
                &parameter_buffer,
            );
            slots.push(FrameSlotResources {
                parameters: FrameParameterArena::new(alignment, parameter_buffer_bytes),
                parameter_buffer,
                bind_groups,
                uses: 0,
            });
        }
        let readback = ReadbackRing::new(
            &context.device,
            &frame,
            plan.canvas.width,
            plan.canvas.height,
            requirements.copy_bytes,
            resource_estimates.packed_frame_bytes,
            pipeline_depth,
        )?;
        let mut stats = decoded.stats().clone();
        stats.source_texture_count = sources.textures.len();
        stats.source_texture_bytes = sources.uploaded_texture_bytes;
        stats.sampler_count = 0;
        stats.uploaded_texture_count = sources.textures.len();
        stats.uploaded_texture_bytes = sources.uploaded_texture_bytes;
        stats.readback_buffer_count = pipeline_depth;
        stats.readback_buffer_bytes = resource_estimates.readback_buffer_bytes;
        stats.shader_module_count = 3;
        stats.pipeline_count = 3;
        stats.output_texture_count = frame.working.texture_count();
        stats.accumulation_buffer_count = 0;
        stats.bind_group_count = per_slot_bind_group_count * pipeline_depth;
        stats.estimated_staging_memory_bytes = resource_estimates.total_staging_bytes;
        let mut timings = decoded.timings();
        timings.gpu_adapter_request = context.adapter_request;
        timings.gpu_device_request = context.device_request;
        timings.gpu_pipeline_creation = pipeline_creation;
        timings.texture_upload = upload_started.elapsed();
        timings.gpu_initialization = started.elapsed();
        context
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        context.device.push_error_scope(wgpu::ErrorFilter::Internal);
        drain(&context.device);
        finish_error_scopes(&context.device, "WGPU-RESOURCE-CREATION")?;
        Ok(Self {
            context,
            pipelines,
            frame,
            sources,
            slots,
            readback,
            pipeline_depth,
            resource_estimates,
            last_execution: FrameExecutionMetrics::default(),
            stats,
            timings,
            staged: StagedMetrics {
                configured_pipeline_depth: pipeline_depth,
                allocated_slot_count: pipeline_depth,
                ..StagedMetrics::default()
            },
            aborted: false,
        })
    }

    fn runtime_context(
        &self,
        diagnostic: Diagnostic,
        token: Option<super::readback::SubmissionToken>,
    ) -> Diagnostic {
        let token = token.map_or_else(
            || "none".to_owned(),
            |token| {
                format!(
                    "frame={} slot={} generation={}",
                    token.frame_number, token.slot_index, token.generation
                )
            },
        );
        diagnostic.with_hint(format!(
            "adapter={} backend={} token={} last_written={} submitted={} in_flight={}",
            self.context.adapter_metadata.adapter_name,
            self.context.adapter_metadata.graphics_backend,
            token,
            self.staged.written_frames,
            self.staged.submitted_frames,
            self.in_flight(),
        ))
    }

    fn process_callbacks_and_take_ready(&mut self) -> Result<Option<CompletedFrame>, Diagnostic> {
        let before = self.readback.metrics();
        self.readback.process_callbacks()?;
        let after = self.readback.metrics();
        self.timings.row_repack += after.row_repack_duration - before.row_repack_duration;
        let ready = self.readback.take_ready();
        if ready.is_some() {
            self.staged.backend_completed_frames += 1;
        }
        Ok(ready)
    }
}

impl RenderBackend for WgpuBackend {
    fn kind(&self) -> RenderBackendKind {
        RenderBackendKind::Wgpu
    }

    fn capacity(&self) -> usize {
        self.pipeline_depth
    }

    fn in_flight(&self) -> usize {
        self.readback.in_flight()
    }

    fn submit_frame(
        &mut self,
        frame_number: u64,
        evaluated: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        self.context
            .runtime_errors
            .check()
            .map_err(|error| self.runtime_context(error, None))?;
        if self.aborted {
            return Err(Diagnostic::error(
                "WGPU-ABORTED",
                crate::Category::Backend,
                "WGPU backend has been aborted",
                "",
            ));
        }
        let token = self.readback.acquire(frame_number)?;
        let slot = &mut self.slots[token.slot_index];
        if slot.uses > 0 {
            self.staged.parameter_slot_reuse_count += 1;
        }
        slot.uses += 1;
        let plan = GpuFramePlan::build(evaluated);
        if let Err(error) = plan.validate(self.sources.textures.len()).and_then(|()| {
            slot.parameters.reset();
            encode_parameters(&mut slot.parameters, evaluated, &plan, &self.sources)
        }) {
            let error = self.runtime_context(error, Some(token));
            self.abort();
            return Err(error);
        }
        let execution = match encode_and_submit(
            &self.context.device,
            &self.context.queue,
            &self.pipelines,
            &self.frame,
            &slot.bind_groups,
            &plan,
            &slot.parameters,
            &slot.parameter_buffer,
            self.readback.buffer(&token)?,
            evaluated.width,
            evaluated.height,
        ) {
            Ok(execution) => execution,
            Err(error) => {
                let error = self.runtime_context(error, Some(token));
                self.abort();
                return Err(error);
            }
        };
        // Error scopes are intentionally not awaited in the hot submission
        // path: doing so serializes map scheduling and defeats staging.
        let token = self.readback.record_submission(
            &token,
            execution
                .submission_index
                .clone()
                .expect("queue submission index"),
        )?;
        if let Err(error) = self.readback.map_async(&token) {
            let error = self.runtime_context(error, Some(token));
            self.abort();
            return Err(error);
        }
        self.staged.submitted_frames += 1;
        self.staged.peak_frames_in_flight = self.staged.peak_frames_in_flight.max(self.in_flight());
        self.last_execution = execution.clone();
        self.timings.gpu_frame_command_encode += execution.command_encode;
        self.timings.gpu_submission += execution.submission;
        self.stats.command_submission_count += execution.queue_submissions;
        Ok(())
    }

    fn poll_completed(&mut self, mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        self.context
            .runtime_errors
            .check()
            .map_err(|error| self.runtime_context(error, None))?;
        if self.aborted {
            return Ok(None);
        }
        // A ready completion always wins over device progress or a blocking
        // wait. This keeps capacity available and makes WaitForOne observable.
        self.process_callbacks_and_take_ready()
            .map_err(|error| self.runtime_context(error, None))
            .and_then(|ready| {
                if ready.is_some() {
                    return Ok(ready);
                }
                let duration = nonblocking(&self.context.device);
                self.staged.nonblocking_polls += 1;
                self.staged.nonblocking_poll_duration += duration;
                self.context
                    .runtime_errors
                    .check()
                    .map_err(|error| self.runtime_context(error, None))?;
                self.process_callbacks_and_take_ready()
                    .map_err(|error| self.runtime_context(error, None))
            })
            .and_then(|ready| {
                if ready.is_some() || mode == PollMode::NonBlocking || self.in_flight() == 0 {
                    return Ok(ready);
                }
                let duration = match mode {
                    PollMode::WaitForOne => {
                        self.staged.slot_wait_count += 1;
                        self.staged.blocking_polls += 1;
                        wait_for_one(&self.context.device, &self.readback.oldest_token()?)
                    }
                    PollMode::Drain => {
                        self.staged.drain_polls += 1;
                        drain(&self.context.device)
                    }
                    PollMode::NonBlocking => unreachable!(),
                };
                self.staged.poll_wait_duration += duration;
                self.timings.gpu_readback_wait += duration;
                self.process_callbacks_and_take_ready()
                    .map_err(|error| self.runtime_context(error, None))
            })
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        self.context
            .runtime_errors
            .check()
            .map_err(|error| self.runtime_context(error, None))?;
        if self.aborted {
            return Ok(Vec::new());
        }
        let started = Instant::now();
        let mut completed = Vec::new();
        while self.in_flight() > 0 {
            if let Some(frame) = self.poll_completed(PollMode::Drain)? {
                completed.push(frame);
            }
        }
        while let Some(frame) = self.readback.take_ready() {
            self.staged.backend_completed_frames += 1;
            completed.push(frame);
        }
        if !self.readback.all_available() {
            return Err(Diagnostic::error(
                "WGPU-FLUSH-INCOMPLETE",
                crate::Category::Backend,
                "WGPU flush left a readback slot unavailable",
                "",
            ));
        }
        self.staged.flush_duration += started.elapsed();
        Ok(completed)
    }

    fn abort(&mut self) {
        if self.aborted {
            return;
        }
        let started = Instant::now();
        self.aborted = true;
        self.readback.abort();
        self.staged.abort_drain_duration += started.elapsed();
    }

    fn stats(&mut self) -> PreparationStats {
        debug_assert_eq!(
            self.stats.source_texture_bytes,
            self.resource_estimates.source_texture_bytes
        );
        debug_assert_eq!(
            self.stats.readback_buffer_bytes,
            self.resource_estimates.readback_buffer_bytes
        );
        self.stats.clone()
    }

    fn timings(&self) -> PreparationTimings {
        self.timings
    }

    fn staged_metrics(&self) -> StagedMetrics {
        let mut metrics = self.staged;
        let readback = self.readback.metrics();
        metrics.map_callback_duration = readback.callback_duration;
        metrics.row_repack_duration = readback.row_repack_duration;
        metrics.mapping_failure_count = readback.mapping_failure_count;
        metrics
    }

    fn record_written(&mut self, _frame_number: u64) {
        self.staged.written_frames += 1;
    }

    fn record_ready_queue(&mut self, length: usize, out_of_order: bool) {
        self.staged.ordered_ready_queue_peak = self.staged.ordered_ready_queue_peak.max(length);
        if out_of_order {
            self.staged.out_of_order_completion_count += 1;
        }
    }

    fn adapter(&self) -> Option<AdapterMetadata> {
        Some(self.context.adapter_metadata.clone())
    }
}

#[cfg(test)]
impl WgpuBackend {
    pub(crate) fn render_frame(
        &mut self,
        frame: &EvaluatedFrame,
        destination: &mut RgbaImage,
    ) -> Result<(), Diagnostic> {
        self.submit_frame(0, frame)?;
        let completed = self.flush()?.into_iter().next().ok_or_else(|| {
            Diagnostic::error(
                "WGPU-READBACK",
                crate::Category::Backend,
                "WGPU completion missing",
                "",
            )
        })?;
        destination.copy_from_slice(&completed.rgba);
        Ok(())
    }

    pub(super) fn last_execution_metrics(&self) -> FrameExecutionMetrics {
        self.last_execution.clone()
    }

    pub(super) fn resource_estimates(&self) -> ResourceEstimates {
        self.resource_estimates
    }
}

const DEFAULT_PIPELINE_DEPTH: usize = 3;
const MAX_PIPELINE_DEPTH: usize = 3;

fn pipeline_depth_from_environment() -> Result<usize, Diagnostic> {
    let value = std::env::var("VIDEO_EDITOR_WGPU_IN_FLIGHT")
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| invalid_pipeline_depth(&value))
        })
        .transpose()?
        .unwrap_or(DEFAULT_PIPELINE_DEPTH);
    validate_pipeline_depth(value)?;
    Ok(value)
}

fn validate_pipeline_depth(value: usize) -> Result<(), Diagnostic> {
    if (1..=MAX_PIPELINE_DEPTH).contains(&value) {
        Ok(())
    } else {
        Err(invalid_pipeline_depth(&value.to_string()))
    }
}

fn invalid_pipeline_depth(value: &str) -> Diagnostic {
    Diagnostic::error(
        "WGPU-IN-FLIGHT",
        crate::Category::Backend,
        format!("pipeline depth must be 1 through {MAX_PIPELINE_DEPTH}, got {value}"),
        "",
    )
}

fn encode_parameters(
    arena: &mut FrameParameterArena,
    frame: &EvaluatedFrame,
    plan: &GpuFramePlan,
    sources: &SourceResources,
) -> Result<(), Diagnostic> {
    for operation in &plan.operations {
        let parameters = match operation {
            GpuOperation::ClearCanvas { .. } => LayerParameters {
                header: [frame.width, frame.height, 0, 0],
                solid_or_background: frame.background.map(f64::from).map(|value| value as f32),
                ..LayerParameters::zeroed()
            },
            GpuOperation::RenderImageLayer {
                layer_index,
                source_asset_index,
                ..
            } => {
                let EvaluatedSource::Image {
                    crop,
                    sizing,
                    transform,
                    cacheable_crop,
                    ..
                } = &frame.layers[*layer_index].source
                else {
                    unreachable!("image frame operation must reference image source")
                };
                let (width, height) = sources.dimensions[*source_asset_index];
                parameters::image(
                    frame,
                    width,
                    height,
                    *crop,
                    *cacheable_crop,
                    sizing,
                    *transform,
                    1.0,
                    crate::plan::ColourTransform::default(),
                )
            }
            GpuOperation::RenderSolidLayer { layer_index, .. } => {
                let EvaluatedSource::SolidColor { colour } = frame.layers[*layer_index].source
                else {
                    unreachable!("solid frame operation must reference solid source")
                };
                LayerParameters {
                    header: [frame.width, frame.height, 0, 2],
                    effective: [0.0, 0.0, 1.0, 0.0],
                    colour_row0: [1.0, 0.0, 0.0, 0.0],
                    colour_row1: [0.0, 1.0, 0.0, 0.0],
                    colour_row2: [0.0, 0.0, 1.0, 0.0],
                    colour_offset: [0.0, 0.0, 0.0, 0.0],
                    solid_or_background: colour.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                }
            }
            GpuOperation::CompositeLayer { layer_index, .. } => LayerParameters {
                header: [
                    frame.width,
                    frame.height,
                    0,
                    blend_mode(frame.layers[*layer_index].blend_mode),
                ],
                effective: [0.0, 0.0, frame.layers[*layer_index].opacity as f32, 0.0],
                ..LayerParameters::zeroed()
            },
            GpuOperation::ApplyEffect { pass, .. } => {
                effect_parameters(frame.width, frame.height, *pass)
            }
            GpuOperation::CopyForEffect { .. } | GpuOperation::CopyForReadback { .. } => {
                continue;
            }
        };
        arena.push(parameters)?;
    }
    Ok(())
}

const fn blend_mode(mode: BlendMode) -> u32 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Add => 1,
        BlendMode::Screen => 2,
        BlendMode::Multiply => 3,
        BlendMode::Overlay => 4,
    }
}

fn effect_parameters(width: u32, height: u32, pass: EffectPass) -> LayerParameters {
    let mut parameters = LayerParameters {
        header: [width, height, 0, 0],
        ..LayerParameters::zeroed()
    };
    match pass {
        EffectPass::ApplyColourTransform { transform } => {
            parameters.header[2] = 1;
            parameters.colour_row0[..3]
                .copy_from_slice(&transform.matrix[0].map(|value| value as f32));
            parameters.colour_row1[..3]
                .copy_from_slice(&transform.matrix[1].map(|value| value as f32));
            parameters.colour_row2[..3]
                .copy_from_slice(&transform.matrix[2].map(|value| value as f32));
            parameters.colour_offset[..3]
                .copy_from_slice(&transform.offset.map(|value| value as f32));
        }
        EffectPass::GaussianHorizontal { radius } => {
            parameters.header[2] = 2;
            parameters.effective[0] = radius as f32;
        }
        EffectPass::GaussianVertical { radius } => {
            parameters.header[2] = 3;
            parameters.effective[0] = radius as f32;
        }
        EffectPass::HighlightExtract { threshold, colour } => {
            parameters.header[2] = 4;
            parameters.effective[0] = threshold as f32;
            parameters.solid_or_background = colour.map(f32::from);
        }
        EffectPass::GlowComposite { intensity } => {
            parameters.header[2] = 5;
            parameters.effective[0] = intensity as f32;
        }
        EffectPass::UnsharpComposite { amount } => {
            parameters.header[2] = 6;
            parameters.effective[0] = amount as f32;
        }
        EffectPass::DirectionalBlur {
            radius,
            angle_degrees,
        } => {
            parameters.header[2] = 7;
            blur_parameters(&mut parameters, radius, angle_degrees, None);
        }
        EffectPass::ZoomBlur {
            radius,
            samples,
            anchor,
            direction,
        } => {
            parameters.header[2] = 8;
            parameters.effective = [
                radius as f32,
                f32::from(samples),
                anchor.x as f32,
                anchor.y as f32,
            ];
            parameters.solid_or_background[0] = match direction {
                ZoomBlurDirection::Centered => 0.0,
                ZoomBlurDirection::Inward => 1.0,
                ZoomBlurDirection::Outward => 2.0,
            };
        }
        EffectPass::ChromaticAberration {
            amount,
            angle_degrees,
        } => {
            parameters.header[2] = 9;
            parameters.effective = [amount as f32, angle_degrees.to_radians() as f32, 0.0, 0.0];
        }
        EffectPass::Vignette {
            amount,
            radius,
            softness,
            colour,
        } => {
            parameters.header[2] = 10;
            parameters.effective = [amount as f32, radius as f32, softness as f32, 0.0];
            parameters.solid_or_background = colour.map(f32::from);
        }
        EffectPass::ColorAdjust {
            exposure,
            gamma,
            black_point,
            white_point,
        } => {
            parameters.header[2] = 11;
            parameters.effective = [
                exposure as f32,
                gamma as f32,
                black_point as f32,
                white_point as f32,
            ];
        }
        EffectPass::MotionBlur {
            radius,
            angle_degrees,
            samples,
        } => {
            parameters.header[2] = 12;
            blur_parameters(&mut parameters, radius, angle_degrees, Some(samples));
        }
    }
    parameters
}

fn blur_parameters(
    parameters: &mut LayerParameters,
    radius: f64,
    angle_degrees: f64,
    configured_samples: Option<u8>,
) {
    let radius = radius.clamp(0.0, 32.0);
    let samples = configured_samples.map_or_else(
        || (radius.ceil() as i32 * 2 + 1).clamp(3, 33) as u8,
        |samples| samples.clamp(1, 33),
    );
    parameters.effective = [
        radius as f32,
        angle_degrees.to_radians() as f32,
        f32::from(samples),
        0.0,
    ];
}

#[cfg(test)]
mod configuration_tests {
    use super::*;

    #[test]
    fn pipeline_depth_accepts_supported_test_depths() {
        for depth in [1, 2, 3] {
            validate_pipeline_depth(depth).expect("supported pipeline depth");
        }
    }

    #[test]
    fn pipeline_depth_rejects_zero_and_excessive_values() {
        for depth in [0, 4, usize::MAX] {
            let error = validate_pipeline_depth(depth).expect_err("invalid pipeline depth");
            assert_eq!(error.code, "WGPU-IN-FLIGHT");
        }
    }
}
