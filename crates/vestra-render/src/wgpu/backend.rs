//! Prepared WGPU texture-frame backend.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use bytemuck::Zeroable;
use image::RgbaImage;

use crate::{
    Diagnostic,
    plan::{EvaluatedFrame, EvaluatedSource, RenderPlan},
    render::{
        AdapterMetadata, ByteLruCache, CompletedFrame, DecodedAssets, PollMode, RenderBackend,
        RenderBackendKind,
        metrics::{PreparationStats, PreparationTimings, StagedMetrics},
    },
};

use super::{
    context::GpuContext,
    diagnostics::{environment_value, finish_error_scopes},
    executor::{
        FrameBindGroups, FrameExecutionMetrics, ParticleUpload, append_particle_upload,
        encode_and_submit,
    },
    frame_plan::{GpuFramePlan, GpuOperation},
    parameters::{self, FrameParameterArena, LayerParameters},
    pipeline::GpuPipelines,
    polling::{drain, nonblocking, wait_for_one},
    readback::ReadbackRing,
    requirements::{GpuRequirements, ResourceEstimates},
    resources::{FrameResources, SourceResources},
    texture_pool::{StaticLayerTexture, create_static_layer_texture, static_layer_texture_bytes},
};
use crate::project::BlendMode;

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
    static_layers: ByteLruCache<usize, Arc<StaticLayerTexture>>,
    static_cache_budget_bypasses: u64,
    static_layer_renders: u64,
    static_cache_population_renders: u64,
    pending_static_layers: PendingStaticLayers,
    temporary_texture_reuses: u64,
}

struct FrameSlotResources {
    parameters: FrameParameterArena,
    parameter_buffer: wgpu::Buffer,
    particle_instances: Vec<super::particles::GpuParticleInstance>,
    particle_pixels: Vec<u8>,
    particle_upload_bytes: Vec<u8>,
    particle_uploads: Vec<Option<ParticleUpload>>,
    particle_buffer: Option<wgpu::Buffer>,
    particle_upload_buffer: Option<wgpu::Buffer>,
    bind_groups: FrameBindGroups,
    uses: u64,
}

struct PendingStaticLayer {
    key: usize,
    bytes: u64,
    texture: Option<Arc<StaticLayerTexture>>,
}

#[derive(Default)]
struct PendingStaticLayers {
    by_submission: BTreeMap<(usize, u64), Vec<PendingStaticLayer>>,
    keys: BTreeSet<usize>,
    reserved_bytes: u64,
}

impl PendingStaticLayers {
    fn contains(&self, key: usize) -> bool {
        self.keys.contains(&key)
    }

    fn reserve(&mut self, token: (usize, u64), key: usize, bytes: u64) {
        assert!(self.keys.insert(key), "static cache key reserved twice");
        self.reserved_bytes += bytes;
        self.by_submission
            .entry(token)
            .or_default()
            .push(PendingStaticLayer {
                key,
                bytes,
                texture: None,
            });
    }

    fn attach(&mut self, token: (usize, u64), key: usize, texture: Arc<StaticLayerTexture>) {
        let entry = self
            .by_submission
            .get_mut(&token)
            .and_then(|entries| entries.iter_mut().find(|entry| entry.key == key))
            .expect("reserved static cache entry is attached to its submission");
        entry.texture = Some(texture);
    }

    fn release(&mut self, token: (usize, u64)) -> Vec<(usize, Arc<StaticLayerTexture>)> {
        let entries = self.by_submission.remove(&token).unwrap_or_default();
        entries
            .into_iter()
            .filter_map(|entry| {
                self.keys.remove(&entry.key);
                self.reserved_bytes -= entry.bytes;
                entry.texture.map(|texture| (entry.key, texture))
            })
            .collect()
    }

    fn clear(&mut self) {
        self.by_submission.clear();
        self.keys.clear();
        self.reserved_bytes = 0;
    }
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
        let requirements =
            GpuRequirements::from_plan(plan, &decoded, parameters::PARAMETER_RECORD_BYTES as u32)?;
        let context = GpuContext::create(plan, requirements)?;
        // Preparation is synchronous and infrequent, so scope errors here can
        // be collected deterministically. Normal frame submission deliberately
        // does not use this path because awaiting scopes serializes staging.
        context
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        context.device.push_error_scope(wgpu::ErrorFilter::Internal);
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
                label: Some("vestra frame parameters"),
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
                particle_instances: Vec::new(),
                particle_pixels: Vec::new(),
                particle_upload_bytes: Vec::new(),
                particle_uploads: Vec::new(),
                particle_buffer: None,
                particle_upload_buffer: None,
                bind_groups,
                uses: 0,
            });
            count
        };
        while slots.len() < pipeline_depth {
            let parameter_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vestra frame parameters"),
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
                particle_instances: Vec::new(),
                particle_pixels: Vec::new(),
                particle_upload_bytes: Vec::new(),
                particle_uploads: Vec::new(),
                particle_buffer: None,
                particle_upload_buffer: None,
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
        stats.shader_module_count = pipelines.shader_module_count();
        stats.pipeline_count = pipelines.pipeline_count();
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
            static_layers: ByteLruCache::new(plan.limits.maximum_cache_bytes),
            static_cache_budget_bypasses: 0,
            static_layer_renders: 0,
            static_cache_population_renders: 0,
            pending_static_layers: PendingStaticLayers::default(),
            temporary_texture_reuses: 0,
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
        Ok(self.take_ready())
    }

    fn take_ready(&mut self) -> Option<CompletedFrame> {
        let (token, ready) = self.readback.take_ready_with_token()?;
        for (key, texture) in self
            .pending_static_layers
            .release((token.slot_index, token.generation))
        {
            self.static_layers
                .insert(key, texture.clone(), texture.estimated_bytes);
        }
        self.staged.backend_completed_frames += 1;
        Some(ready)
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
        if evaluated
            .layers
            .iter()
            .any(|layer| matches!(&layer.source, crate::plan::EvaluatedSource::Group { .. }))
        {
            return Err(Diagnostic::error(
                "WGPU-GROUP-UNSUPPORTED",
                crate::Category::Backend,
                "WGPU Group rendering is not implemented",
                "",
            ));
        }
        let token = self.readback.acquire(frame_number)?;
        let slot = &mut self.slots[token.slot_index];
        if slot.uses > 0 {
            self.staged.parameter_slot_reuse_count += 1;
        }
        slot.uses += 1;
        let mut cached_layers = BTreeSet::new();
        let mut cache_targets = BTreeSet::new();
        let mut textures = BTreeMap::new();
        for layer in &evaluated.layers {
            if layer.content_dependency != crate::plan::TemporalDependency::Static {
                continue;
            }
            let key = layer.compiled_layer_index;
            if self.pending_static_layers.contains(key) {
                self.static_layer_renders += 1;
                continue;
            }
            if let Some(texture) = self.static_layers.get(&key).cloned() {
                cached_layers.insert(key);
                textures.insert(key, texture);
                continue;
            }
            self.static_layer_renders += 1;
            let bytes = static_layer_texture_bytes(evaluated.width, evaluated.height);
            if !self
                .static_layers
                .reserve(bytes, self.pending_static_layers.reserved_bytes)
            {
                self.static_cache_budget_bypasses += 1;
                continue;
            }
            self.pending_static_layers
                .reserve((token.slot_index, token.generation), key, bytes);
            self.static_cache_population_renders += 1;
            let texture = Arc::new(create_static_layer_texture(
                &self.context.device,
                evaluated.width,
                evaluated.height,
            ));
            cache_targets.insert(key);
            self.pending_static_layers.attach(
                (token.slot_index, token.generation),
                key,
                texture.clone(),
            );
            textures.insert(key, texture);
        }
        let mut plan =
            GpuFramePlan::build_with_static_cache(evaluated, &cached_layers, &cache_targets);
        if let Err(error) = plan.validate(self.sources.textures.len()).and_then(|()| {
            slot.parameters.reset();
            encode_parameters(
                &mut slot.parameters,
                evaluated,
                &mut plan,
                &self.sources,
                &mut slot.particle_instances,
                &mut slot.particle_pixels,
                &mut slot.particle_upload_bytes,
                &mut slot.particle_uploads,
            )
        }) {
            let error = self.runtime_context(error, Some(token));
            self.abort();
            return Err(error);
        }
        let mut particle_buffer = slot.particle_buffer.take();
        if !slot.particle_instances.is_empty() {
            let required = slot
                .particle_instances
                .len()
                .checked_mul(std::mem::size_of::<super::particles::GpuParticleInstance>())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or_else(|| {
                    Diagnostic::error(
                        "WGPU-PARTICLE-BUFFER",
                        crate::Category::Backend,
                        "particle instance buffer size overflow",
                        "",
                    )
                })?;
            let replace = particle_buffer
                .as_ref()
                .is_none_or(|buffer| buffer.size() < required);
            if replace {
                particle_buffer =
                    Some(self.context.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("vestra particle instances"),
                        size: required.next_power_of_two().max(32),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
            }
            self.context.queue.write_buffer(
                particle_buffer.as_ref().expect("particle buffer created"),
                0,
                bytemuck::cast_slice(&slot.particle_instances),
            );
        }
        slot.particle_buffer = particle_buffer;
        let mut particle_upload_buffer = slot.particle_upload_buffer.take();
        if !slot.particle_upload_bytes.is_empty() {
            let required = u64::try_from(slot.particle_upload_bytes.len()).map_err(|_| {
                Diagnostic::error(
                    "WGPU-PARTICLE-UPLOAD",
                    crate::Category::Backend,
                    "particle upload size overflow",
                    "",
                )
            })?;
            if particle_upload_buffer
                .as_ref()
                .is_none_or(|buffer| buffer.size() < required)
            {
                particle_upload_buffer =
                    Some(self.context.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("vestra additive particle uploads"),
                        size: required.next_power_of_two().max(32),
                        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
            }
            self.context.queue.write_buffer(
                particle_upload_buffer
                    .as_ref()
                    .expect("particle upload buffer created"),
                0,
                &slot.particle_upload_bytes,
            );
        }
        slot.particle_upload_buffer = particle_upload_buffer;
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
            &textures,
            evaluated.width,
            evaluated.height,
            slot.particle_buffer.as_ref(),
            slot.particle_upload_buffer.as_ref(),
            &slot.particle_uploads,
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
        if self.staged.submitted_frames > 0 {
            // This is a structural slot count, not a record of textures used
            // by this frame's plan. Every later submission reuses the fixed
            // prepared working set.
            self.temporary_texture_reuses += self.frame.working.texture_count() as u64;
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

    fn poll_completed_cancellable(
        &mut self,
        mode: PollMode,
        cancelled: &AtomicBool,
    ) -> Result<Option<CompletedFrame>, Diagnostic> {
        if mode == PollMode::NonBlocking {
            return self.poll_completed(mode);
        }
        // WGPU's blocking Maintain calls cannot be interrupted. Poll in a
        // bounded cadence instead so cancellation can stop writes promptly;
        // this is not a render timeout and makes no assumptions about GPU speed.
        while self.in_flight() > 0 {
            if cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            if let Some(frame) = self.poll_completed(PollMode::NonBlocking)? {
                return Ok(Some(frame));
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(None)
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
        while let Some(frame) = self.take_ready() {
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
        self.pending_static_layers.clear();
        self.staged.abort_drain_duration += started.elapsed();
    }

    fn verify_idle(&self) -> Result<(), Diagnostic> {
        if self.aborted {
            return Err(Diagnostic::error(
                "MVP-BACKEND-NOT-IDLE",
                crate::Category::Backend,
                "WGPU backend was aborted",
                "",
            ));
        }
        if self.in_flight() != 0 || !self.readback.all_available() {
            return Err(Diagnostic::error(
                "MVP-BACKEND-NOT-IDLE",
                crate::Category::Backend,
                "WGPU backend retained pending readback work after flush",
                "",
            ));
        }
        Ok(())
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
        let cache = self.static_layers.stats();
        self.stats.static_cache_hits = cache.hits;
        self.stats.static_cache_misses = cache.misses;
        self.stats.static_cache_entries = cache.current_entries;
        self.stats.static_cached_bytes = cache.current_bytes;
        self.stats.static_cache_budget_bypasses =
            cache.oversized_entries_skipped + self.static_cache_budget_bypasses;
        self.stats.static_cache_population_renders = self.static_cache_population_renders;
        self.stats.static_layers_rendered = self.static_layer_renders;
        self.stats.wgpu_temporary_texture_allocations = self.frame.working.texture_count() as u64;
        self.stats.wgpu_temporary_texture_reuses = self.temporary_texture_reuses;
        self.stats.wgpu_temporary_texture_estimated_bytes = self.frame.working.estimated_bytes();
        self.stats.wgpu_temporary_textures_retained = self.frame.working.texture_count();
        let readback = self.readback.metrics();
        self.stats.readback_tight_rgba_allocations = readback.tight_rgba_allocations;
        self.stats.readback_repack_bytes = readback.repack_bytes;
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
        metrics.submission_to_map_ready = readback.submission_to_map_ready;
        metrics.slot_lifetime = readback.slot_lifetime;
        metrics.mapping_failure_count = readback.mapping_failure_count;
        metrics
    }

    fn reset_operation_metrics(&mut self) {
        self.staged = StagedMetrics {
            configured_pipeline_depth: self.pipeline_depth,
            allocated_slot_count: self.pipeline_depth,
            ..StagedMetrics::default()
        };
        self.readback.reset_metrics();
        self.timings.gpu_frame_command_encode = Duration::ZERO;
        self.timings.gpu_submission = Duration::ZERO;
        self.timings.gpu_readback_wait = Duration::ZERO;
        self.timings.row_repack = Duration::ZERO;
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

    pub(super) fn pending_static_cache_state(&self) -> (usize, u64) {
        (
            self.pending_static_layers.keys.len(),
            self.pending_static_layers.reserved_bytes,
        )
    }
}

const DEFAULT_PIPELINE_DEPTH: usize = 3;
const MAX_PIPELINE_DEPTH: usize = 3;

fn pipeline_depth_from_environment() -> Result<usize, Diagnostic> {
    let value = environment_value("VESTRA_WGPU_IN_FLIGHT", "VIDEO_EDITOR_WGPU_IN_FLIGHT")
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

#[expect(
    clippy::too_many_arguments,
    reason = "parameter preparation receives reusable frame-slot arenas separately to preserve their lifetimes"
)]
fn encode_parameters(
    arena: &mut FrameParameterArena,
    frame: &EvaluatedFrame,
    plan: &mut GpuFramePlan,
    sources: &SourceResources,
    particle_instances: &mut Vec<super::particles::GpuParticleInstance>,
    particle_pixels: &mut Vec<u8>,
    particle_upload_bytes: &mut Vec<u8>,
    particle_uploads: &mut Vec<Option<ParticleUpload>>,
) -> Result<(), Diagnostic> {
    particle_instances.clear();
    particle_upload_bytes.clear();
    particle_uploads.clear();
    for operation in &mut plan.operations {
        match operation {
            GpuOperation::ClearCanvas { .. } => {
                arena.push(&LayerParameters {
                    header: [frame.width, frame.height, 0, 0],
                    solid_or_background: frame.background.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                })?;
            }
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
                arena.push(&parameters::image(
                    frame,
                    width,
                    height,
                    *crop,
                    *cacheable_crop,
                    sizing,
                    *transform,
                    1.0,
                    crate::plan::ColourTransform::default(),
                ))?;
            }
            GpuOperation::RenderSolidLayer { layer_index, .. } => {
                let EvaluatedSource::SolidColor { colour } = frame.layers[*layer_index].source
                else {
                    unreachable!("solid frame operation must reference solid source")
                };
                arena.push(&LayerParameters {
                    header: [frame.width, frame.height, 0, 2],
                    effective: [0.0, 0.0, 1.0, 0.0],
                    colour_row0: [1.0, 0.0, 0.0, 0.0],
                    colour_row1: [0.0, 1.0, 0.0, 0.0],
                    colour_row2: [0.0, 0.0, 1.0, 0.0],
                    colour_offset: [0.0, 0.0, 0.0, 0.0],
                    solid_or_background: colour.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                })?;
            }
            GpuOperation::RenderSpectrum2DLayer { layer_index, .. } => {
                let EvaluatedSource::Spectrum2D {
                    bands,
                    x,
                    y,
                    width,
                    height,
                    bar_gap_ratio,
                    min_bar_height_ratio,
                    layout,
                    gradient,
                    colour,
                } = &frame.layers[*layer_index].source
                else {
                    unreachable!("Spectrum2D frame operation must reference Spectrum2D source")
                };
                arena.push(&parameters::spectrum2d(
                    frame,
                    bands,
                    *x,
                    *y,
                    *width,
                    *height,
                    *bar_gap_ratio,
                    *min_bar_height_ratio,
                    layout,
                    *gradient,
                    *colour,
                )?)?;
            }
            GpuOperation::RenderParticleLayer {
                layer_index,
                instance_offset,
                instance_count,
                ..
            } => {
                let EvaluatedSource::ParticleSystem {
                    system,
                    time_nanos,
                    appearance,
                } = &frame.layers[*layer_index].source
                else {
                    unreachable!("particle frame operation must reference particle source")
                };
                particle_uploads.push(None);
                *instance_offset = u32::try_from(particle_instances.len()).map_err(|_| {
                    Diagnostic::error(
                        "WGPU-PARTICLE-COUNT",
                        crate::Category::Backend,
                        "particle instance count exceeds WGPU draw range",
                        "",
                    )
                })?;
                if matches!(
                    system.blend_mode,
                    crate::project::ParticleBlendMode::Additive
                ) {
                    // CPU Additive is a saturated straight-alpha blend. Portable
                    // fixed-function WGPU blending cannot express that equation
                    // while preserving ordered overlap in one draw, so this is
                    // the exact compatibility path. Effects and outer composition
                    // still run on WGPU after this source upload.
                    let pixel_len = usize::try_from(frame.width)
                        .ok()
                        .and_then(|width| {
                            usize::try_from(frame.height)
                                .ok()
                                .and_then(|height| width.checked_mul(height))
                        })
                        .and_then(|pixels| pixels.checked_mul(4))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "WGPU-PARTICLE-UPLOAD",
                                crate::Category::Backend,
                                "particle image size overflow",
                                "",
                            )
                        })?;
                    particle_pixels.resize(pixel_len, 0);
                    particle_pixels.fill(0);
                    let mut image = RgbaImage::from_raw(
                        frame.width,
                        frame.height,
                        std::mem::take(particle_pixels),
                    )
                    .expect("validated particle image dimensions");
                    crate::cpu::particles::rasterize_instances(
                        &mut image,
                        system.evaluated_particles_at_with_appearance(*time_nanos, *appearance),
                        system.primitive,
                        system.blend_mode,
                        crate::plan::ColourTransform::default(),
                    );
                    let pixels = image.into_raw();
                    let upload = append_particle_upload(
                        particle_upload_bytes,
                        &pixels,
                        frame.width,
                        frame.height,
                    )?;
                    particle_uploads
                        .last_mut()
                        .expect("upload entry exists")
                        .replace(upload);
                    *particle_pixels = pixels;
                    *instance_count = 0;
                    arena.push(&parameters::particles(frame, system.primitive))?;
                    continue;
                }
                let range = super::particles::pack_into(
                    particle_instances,
                    system,
                    *time_nanos,
                    *appearance,
                )?;
                *instance_count = u32::try_from(range.len()).map_err(|_| {
                    Diagnostic::error(
                        "WGPU-PARTICLE-COUNT",
                        crate::Category::Backend,
                        "particle instance count exceeds WGPU draw range",
                        "",
                    )
                })?;
                arena.push(&parameters::particles(frame, system.primitive))?;
            }
            GpuOperation::CompositeLayer { layer_index, .. }
            | GpuOperation::CompositeCachedLayer { layer_index, .. } => {
                arena.push(&LayerParameters {
                    header: [
                        frame.width,
                        frame.height,
                        0,
                        blend_mode(frame.layers[*layer_index].blend_mode),
                    ],
                    effective: [0.0, 0.0, frame.layers[*layer_index].opacity as f32, 0.0],
                    ..LayerParameters::zeroed()
                })?;
            }
            GpuOperation::ApplyEffect { kernel, pass, .. } => {
                let parameters = parameters::effect_parameters(frame.width, frame.height, *pass);
                if parameters.kernel() != *kernel {
                    return Err(Diagnostic::error(
                        "WGPU-EFFECT-KERNEL-MISMATCH",
                        crate::Category::Backend,
                        "encoded effect parameters do not match the frame-plan kernel",
                        "",
                    ));
                }
                parameters::push_effect_parameters(arena, parameters)?;
            }
            GpuOperation::CopyForEffect { .. }
            | GpuOperation::ResolveParticleLayer { .. }
            | GpuOperation::StoreStaticLayer { .. }
            | GpuOperation::CopyForReadback { .. } => {
                continue;
            }
        }
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

#[cfg(test)]
mod configuration_tests {
    use super::*;
    use crate::render::effects::{CompositeMode, EffectOperation, EffectPass};

    #[test]
    fn effect_parameter_encodings_use_semantic_typed_layouts() {
        let gaussian = parameters::effect_parameters(
            320,
            180,
            EffectPass::new(
                EffectOperation::GaussianVertical { radius: 4.0 },
                crate::plan::EffectResource::Current,
                crate::plan::EffectResource::Current,
            ),
        );
        let parameters::EffectKernelParameters::GaussianBlur(gaussian) = gaussian else {
            panic!("Gaussian pass must use the Gaussian layout");
        };
        assert_eq!((gaussian.canvas_width, gaussian.canvas_height), (320, 180));
        assert_eq!(gaussian.radius, 4.0);
        assert_eq!(gaussian.direction, 1);

        let composite = parameters::effect_parameters(
            320,
            180,
            EffectPass::new(
                EffectOperation::Composite {
                    mode: CompositeMode::Additive,
                    amount: 0.75,
                },
                crate::plan::EffectResource::Original,
                crate::plan::EffectResource::Current,
            ),
        );
        let parameters::EffectKernelParameters::Composite(composite) = composite else {
            panic!("Composite pass must use the Composite layout");
        };
        assert_eq!(composite.mode, 0);
        assert_eq!(composite.amount, 0.75);

        let vignette = parameters::effect_parameters(
            320,
            180,
            EffectPass::new(
                EffectOperation::Vignette {
                    amount: 0.5,
                    radius: 0.6,
                    softness: 0.2,
                    colour: [10, 20, 30, 255],
                },
                crate::plan::EffectResource::Current,
                crate::plan::EffectResource::Current,
            ),
        );
        let parameters::EffectKernelParameters::Vignette(vignette) = vignette else {
            panic!("Vignette pass must use the Vignette layout");
        };
        assert_eq!(
            (vignette.amount, vignette.radius, vignette.softness),
            (0.5, 0.6, 0.2)
        );
        assert_eq!(vignette.colour, [10.0, 20.0, 30.0, 255.0]);
        assert_eq!(
            std::mem::size_of::<parameters::GaussianBlurParameters>() % 16,
            0
        );
        assert_eq!(
            std::mem::size_of::<parameters::VignetteParameters>() % 16,
            0
        );
        assert!(
            std::mem::size_of::<parameters::GaussianBlurParameters>()
                < std::mem::size_of::<LayerParameters>()
        );
    }

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

    #[test]
    fn pending_static_keys_reserve_once_and_release_on_completion_or_abort() {
        let mut pending = PendingStaticLayers::default();
        pending.reserve((0, 1), 7, 16);
        assert!(pending.contains(7));
        assert_eq!(pending.reserved_bytes, 16);
        assert!(!pending.contains(8));
        assert!(pending.release((0, 1)).is_empty());
        assert!(!pending.contains(7));
        assert_eq!(pending.reserved_bytes, 0);

        pending.reserve((1, 2), 7, 16);
        pending.clear();
        assert!(!pending.contains(7));
        assert_eq!(pending.reserved_bytes, 0);
    }
}
