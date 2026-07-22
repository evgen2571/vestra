//! Headless WGPU resource ownership for the render backend.

#![allow(
    clippy::result_large_err,
    reason = "WGPU preparation retains structured user-facing diagnostics"
)]

use std::{sync::Arc, time::Instant};

use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use serde::Serialize;

use crate::{
    Category, Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, RenderBackend, RenderBackendKind,
        prepared::{DecodedAssets, PreparationStats, PreparationTimings, crop_bounds},
    },
};

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
    _accumulation: wgpu::Buffer,
    _source_textures: Vec<wgpu::Texture>,
    _source_bind_groups: Vec<wgpu::BindGroup>,
    _solid_texture: wgpu::Texture,
    solid_bind_group: wgpu::BindGroup,
    source_dimensions: Vec<(u32, u32)>,
    output: wgpu::Texture,
    readback: wgpu::Buffer,
    row_bytes: u32,
    padded_row_bytes: u32,
    frame_bytes: Vec<u8>,
    stats: PreparationStats,
    timings: PreparationTimings,
    adapter: AdapterMetadata,
}

/// Matches the explicit sixteen-byte chunks in `layer.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LayerParameters {
    header: [u32; 4],
    source: [u32; 4],
    crop: [f32; 4],
    effective: [f32; 4],
    inverse_row0: [f32; 4],
    inverse_row1: [f32; 4],
    colour_row0: [f32; 4],
    colour_row1: [f32; 4],
    colour_row2: [f32; 4],
    colour_offset: [f32; 4],
    solid_or_background: [f32; 4],
}

/// Quantitative CPU/GPU frame comparison used by parity fixtures.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct FrameDifference {
    pub maximum_absolute_channel_error: u8,
    pub mean_absolute_channel_error: f64,
    pub differing_channels: usize,
    pub differing_channel_percentage: f64,
    pub channels_exceeding_tolerance: usize,
    pub pixels_exceeding_tolerance: usize,
}

#[must_use]
pub fn compare_rgba(reference: &[u8], candidate: &[u8], tolerance: u8) -> FrameDifference {
    assert_eq!(
        reference.len(),
        candidate.len(),
        "frame buffers must have equal size"
    );
    let mut difference = FrameDifference::default();
    let mut total = 0_u64;
    for (reference_pixel, candidate_pixel) in
        reference.chunks_exact(4).zip(candidate.chunks_exact(4))
    {
        let mut pixel_exceeds_tolerance = false;
        for (&left, &right) in reference_pixel.iter().zip(candidate_pixel) {
            let error = left.abs_diff(right);
            difference.maximum_absolute_channel_error =
                difference.maximum_absolute_channel_error.max(error);
            total += u64::from(error);
            difference.differing_channels += usize::from(error != 0);
            difference.channels_exceeding_tolerance += usize::from(error > tolerance);
            pixel_exceeds_tolerance |= error > tolerance;
        }
        difference.pixels_exceeding_tolerance += usize::from(pixel_exceeds_tolerance);
    }
    difference.mean_absolute_channel_error = if reference.is_empty() {
        0.0
    } else {
        total as f64 / reference.len() as f64
    };
    difference.differing_channel_percentage = if reference.is_empty() {
        0.0
    } else {
        difference.differing_channels as f64 / reference.len() as f64 * 100.0
    };
    difference
}

impl WgpuBackend {
    pub fn new(plan: &RenderPlan, decoded: Arc<DecodedAssets>) -> Result<Self, Diagnostic> {
        let started = Instant::now();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: requested_backends(),
            ..wgpu::InstanceDescriptor::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: std::env::var_os("VIDEO_EDITOR_WGPU_FORCE_FALLBACK").is_some(),
            compatible_surface: None,
        }))
        .ok_or_else(|| {
            Diagnostic::error(
                "WGPU-ADAPTER-NOT-FOUND",
                Category::Backend,
                "WGPU adapter request returned no compatible adapter",
                "",
            )
        })?;
        let info = adapter.get_info();
        let adapter_metadata = AdapterMetadata {
            adapter_name: info.name,
            device_type: format!("{:?}", info.device_type).to_lowercase(),
            graphics_backend: format!("{:?}", info.backend).to_lowercase(),
            driver_name: info.driver,
            driver_info: info.driver_info,
            vendor_id: info.vendor,
            device_id: info.device,
        };
        let limits = adapter.limits();
        if plan.canvas.width > limits.max_texture_dimension_2d
            || plan.canvas.height > limits.max_texture_dimension_2d
        {
            return Err(Diagnostic::error(
                "WGPU-OUTPUT-DIMENSIONS",
                Category::Backend,
                format!(
                    "output {}x{} exceeds adapter maximum 2D texture dimension {}",
                    plan.canvas.width, plan.canvas.height, limits.max_texture_dimension_2d
                ),
                "",
            ));
        }
        let requested_row_bytes = plan.canvas.width.checked_mul(4).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-READBACK-SIZE",
                Category::Backend,
                "output row size overflow",
                "",
            )
        })?;
        let requested_copy_size = u64::from(align_up(
            requested_row_bytes,
            wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
        ))
        .checked_mul(u64::from(plan.canvas.height))
        .ok_or_else(|| {
            Diagnostic::error(
                "WGPU-READBACK-SIZE",
                Category::Backend,
                "readback buffer size overflow",
                "",
            )
        })?;
        if requested_copy_size > limits.max_buffer_size {
            return Err(Diagnostic::error(
                "WGPU-BUFFER-LIMIT",
                Category::Backend,
                format!(
                    "output accumulation requires {requested_copy_size} bytes but adapter limit is {}",
                    limits.max_buffer_size
                ),
                "",
            ));
        }
        if requested_copy_size > u64::from(limits.max_storage_buffer_binding_size) {
            return Err(Diagnostic::error(
                "WGPU-STORAGE-LIMIT",
                Category::Backend,
                format!(
                    "output accumulation requires {requested_copy_size} bytes but storage binding limit is {}",
                    limits.max_storage_buffer_binding_size
                ),
                "",
            ));
        }
        if plan.canvas.width.div_ceil(8) > limits.max_compute_workgroups_per_dimension
            || plan.canvas.height.div_ceil(8) > limits.max_compute_workgroups_per_dimension
        {
            return Err(Diagnostic::error(
                "WGPU-DISPATCH-LIMIT",
                Category::Backend,
                "output dispatch exceeds adapter workgroup dimension limit",
                "",
            ));
        }
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("video-editor headless renderer"),
                required_features: wgpu::Features::empty(),
                // Resource requirements above were checked against this
                // adapter. Request that same capability set so the device
                // cannot silently negotiate unrelated downlevel defaults
                // after successful adapter validation.
                required_limits: limits.clone(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|error| diagnostic("WGPU-DEVICE-REQUEST", "device_request", error))?;
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        device.push_error_scope(wgpu::ErrorFilter::Internal);
        let row_bytes = plan.canvas.width.checked_mul(4).ok_or_else(|| {
            Diagnostic::error(
                "WGPU-READBACK-SIZE",
                Category::Backend,
                "output row size overflow",
                "",
            )
        })?;
        let layer_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video-editor layer compute shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/layer.wgsl").into()),
        });
        let layer_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("video-editor layer bindings"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<
                                LayerParameters,
                            >()
                                as u64),
                        },
                        count: None,
                    },
                ],
            });
        let layer_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("video-editor layer pipeline layout"),
                bind_group_layouts: &[&layer_bind_group_layout],
                push_constant_ranges: &[],
            });
        let layer_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("video-editor layer pipeline"),
            layout: Some(&layer_pipeline_layout),
            module: &layer_shader,
            entry_point: "compose",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let padded_row_bytes = align_up(row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback_size = u64::from(padded_row_bytes)
            .checked_mul(u64::from(plan.canvas.height))
            .ok_or_else(|| {
                Diagnostic::error(
                    "WGPU-READBACK-SIZE",
                    Category::Backend,
                    "readback buffer size overflow",
                    "",
                )
            })?;
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video-editor output"),
            size: wgpu::Extent3d {
                width: plan.canvas.width,
                height: plan.canvas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-editor readback"),
            size: readback_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let accumulation = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-editor layer accumulation"),
            size: readback_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let layer_parameters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-editor layer parameters"),
            size: std::mem::size_of::<LayerParameters>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let upload_started = Instant::now();
        let mut source_textures = Vec::with_capacity(plan.images.len());
        let mut source_dimensions = Vec::with_capacity(plan.images.len());
        let mut uploaded_texture_bytes = 0_u64;
        for asset in 0..plan.images.len() {
            let image = decoded.image(asset);
            if image.width() > limits.max_texture_dimension_2d
                || image.height() > limits.max_texture_dimension_2d
            {
                return Err(Diagnostic::error(
                    "WGPU-SOURCE-DIMENSIONS",
                    Category::Backend,
                    format!("source image {} exceeds adapter texture dimensions", asset),
                    "",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("video-editor source"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width() * 4),
                    rows_per_image: Some(image.height()),
                },
                wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
            );
            uploaded_texture_bytes = uploaded_texture_bytes
                .saturating_add(u64::from(image.width()) * u64::from(image.height()) * 4);
            source_textures.push(texture);
            source_dimensions.push((image.width(), image.height()));
        }
        let source_bind_groups: Vec<wgpu::BindGroup> = source_textures
            .iter()
            .map(|texture| {
                create_layer_bind_group(
                    &device,
                    &layer_bind_group_layout,
                    &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    &accumulation,
                    &layer_parameters,
                )
            })
            .collect();
        let solid_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video-editor solid source"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let solid_bind_group = create_layer_bind_group(
            &device,
            &layer_bind_group_layout,
            &solid_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            &accumulation,
            &layer_parameters,
        );
        let mut stats = decoded.stats().clone();
        stats.uploaded_texture_count = source_textures.len();
        stats.uploaded_texture_bytes = uploaded_texture_bytes;
        stats.readback_buffer_count = 1;
        stats.readback_buffer_bytes = readback_size;
        stats.shader_module_count = 1;
        stats.pipeline_count = 1;
        stats.output_texture_count = 1;
        stats.accumulation_buffer_count = 1;
        stats.bind_group_count = source_bind_groups.len() + 1;
        let mut timings = decoded.timings();
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
            _accumulation: accumulation,
            _source_textures: source_textures,
            _source_bind_groups: source_bind_groups,
            _solid_texture: solid_texture,
            solid_bind_group,
            source_dimensions,
            output,
            readback,
            row_bytes,
            padded_row_bytes,
            frame_bytes: vec![0; (u64::from(row_bytes) * u64::from(plan.canvas.height)) as usize],
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

    fn prepare(
        &mut self,
        _plan: &RenderPlan,
        _decoded: Arc<DecodedAssets>,
    ) -> Result<(), Diagnostic> {
        Ok(())
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
        let command_encode_started = Instant::now();
        // Parameter updates and dispatch order are derived solely from the
        // evaluated frame. Each submission observes its matching uniform data.
        let clear = LayerParameters {
            header: [frame.width, frame.height, self.padded_row_bytes / 4, 0],
            solid_or_background: frame.background.map(f64::from).map(|value| value as f32),
            ..LayerParameters::zeroed()
        };
        self.dispatch_layer(&self.solid_bind_group, clear, frame.width, frame.height);
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
                let (source_width, source_height) = self.source_dimensions[*asset_index];
                let parameters = image_parameters(
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
                self.dispatch_layer(
                    &self._source_bind_groups[*asset_index],
                    parameters,
                    frame.width,
                    frame.height,
                );
            } else if let crate::plan::EvaluatedSource::SolidColor { colour } = layer.source {
                let parameters = LayerParameters {
                    header: [frame.width, frame.height, self.padded_row_bytes / 4, 2],
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
                self.dispatch_layer(
                    &self.solid_bind_group,
                    parameters,
                    frame.width,
                    frame.height,
                );
            }
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("video-editor readback copy"),
            });
        encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                buffer: &self._accumulation,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row_bytes),
                    rows_per_image: Some(frame.height),
                },
            },
            wgpu::ImageCopyTexture {
                texture: &self.output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.readback,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row_bytes),
                    rows_per_image: Some(frame.height),
                },
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        self.timings.gpu_frame_command_encode += command_encode_started.elapsed();
        let submission_started = Instant::now();
        self.queue.submit(Some(encoder.finish()));
        self.timings.gpu_submission += submission_started.elapsed();
        self.stats.command_submission_count += frame.layers.len() as u64 + 2;
        let slice = self.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        let readback_wait_started = Instant::now();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        let readback_result = receiver
            .recv()
            .map_err(|error| {
                Diagnostic::error(
                    "WGPU-READBACK",
                    Category::Backend,
                    format!("readback callback failed: {error}"),
                    "",
                )
            })
            .and_then(|result| {
                result.map_err(|error| diagnostic("WGPU-READBACK", "buffer_map", error))
            });
        let frame_error_result = finish_error_scopes(&self.device, "WGPU-COMMAND-SUBMISSION");
        self.timings.gpu_readback_wait += readback_wait_started.elapsed();
        readback_result?;
        if let Err(error) = frame_error_result {
            self.readback.unmap();
            return Err(error);
        }
        let mapped = slice.get_mapped_range();
        let row_repack_started = Instant::now();
        for (row, target) in self
            .frame_bytes
            .chunks_exact_mut(self.row_bytes as usize)
            .enumerate()
        {
            let start = row * self.padded_row_bytes as usize;
            target.copy_from_slice(&mapped[start..start + self.row_bytes as usize]);
        }
        drop(mapped);
        self.readback.unmap();
        self.timings.row_repack += row_repack_started.elapsed();
        destination.as_mut().copy_from_slice(&self.frame_bytes);
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

impl WgpuBackend {
    fn dispatch_layer(
        &self,
        bind_group: &wgpu::BindGroup,
        parameters: LayerParameters,
        width: u32,
        height: u32,
    ) {
        self.queue
            .write_buffer(&self._layer_parameters, 0, bytemuck::bytes_of(&parameters));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("video-editor layer dispatch"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("video-editor layer pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self._layer_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        self.queue.submit(Some(encoder.finish()));
    }
}

fn finish_error_scopes(device: &wgpu::Device, code: &str) -> Result<(), Diagnostic> {
    let internal_error = pollster::block_on(device.pop_error_scope());
    let validation_error = pollster::block_on(device.pop_error_scope());
    if let Some(error) = internal_error {
        return Err(diagnostic(code, "internal", error));
    }
    if let Some(error) = validation_error {
        return Err(diagnostic(code, "validation", error));
    }
    Ok(())
}

fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn requested_backends() -> wgpu::Backends {
    match std::env::var("VIDEO_EDITOR_WGPU_BACKEND")
        .ok()
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("vulkan") => wgpu::Backends::VULKAN,
        Some("gl") | Some("gles") => wgpu::Backends::GL,
        Some("metal") => wgpu::Backends::METAL,
        Some("dx12") => wgpu::Backends::DX12,
        Some("browser_webgpu") => wgpu::Backends::BROWSER_WEBGPU,
        _ => wgpu::Backends::all(),
    }
}

fn create_layer_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &wgpu::TextureView,
    accumulation: &wgpu::Buffer,
    parameters: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("video-editor source layer bindings"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: accumulation.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.as_entire_binding(),
            },
        ],
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the evaluator's image layer fields remain separate to avoid a GPU-specific plan type"
)]
fn image_parameters(
    frame: &EvaluatedFrame,
    source_width: u32,
    source_height: u32,
    crop: crate::domain::Crop,
    cacheable_crop: bool,
    sizing: &crate::plan::CompiledSizing,
    transform: crate::animation::Transform2D,
    opacity: f64,
    colour: crate::plan::ColourTransform,
) -> LayerParameters {
    let (virtual_width, virtual_height, origin_x, origin_y, virtual_crop) = if cacheable_crop {
        let bounds = crop_bounds(source_width, source_height, crop);
        (
            bounds.width,
            bounds.height,
            bounds.x,
            bounds.y,
            crate::domain::Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        )
    } else {
        (source_width, source_height, 0, 0, crop)
    };
    let cropped_width = virtual_crop.width * f64::from(virtual_width);
    let cropped_height = virtual_crop.height * f64::from(virtual_height);
    let (effective_width, effective_height) = match sizing {
        crate::plan::CompiledSizing::Original => (cropped_width, cropped_height),
        crate::plan::CompiledSizing::Stretch { width, height } => {
            (f64::from(*width), f64::from(*height))
        }
        crate::plan::CompiledSizing::Scale(scale) => {
            (cropped_width * scale, cropped_height * scale)
        }
        crate::plan::CompiledSizing::Fit | crate::plan::CompiledSizing::Cover => {
            let horizontal = f64::from(frame.width) / cropped_width;
            let vertical = f64::from(frame.height) / cropped_height;
            let factor = if matches!(sizing, crate::plan::CompiledSizing::Fit) {
                horizontal.min(vertical)
            } else {
                horizontal.max(vertical)
            };
            (cropped_width * factor, cropped_height * factor)
        }
    };
    let (sine, cosine) = transform.rotation_radians.sin_cos();
    let destination_x = transform.position.x * f64::from(frame.width);
    let destination_y = transform.position.y * f64::from(frame.height);
    let anchor_x = transform.anchor.x * effective_width;
    let anchor_y = transform.anchor.y * effective_height;
    let m00 = cosine / transform.scale.x;
    let m01 = sine / transform.scale.x;
    let m10 = -sine / transform.scale.y;
    let m11 = cosine / transform.scale.y;
    LayerParameters {
        header: [
            frame.width,
            frame.height,
            align_up(frame.width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) / 4,
            1,
        ],
        source: [virtual_width, virtual_height, origin_x, origin_y],
        crop: [
            virtual_crop.x as f32,
            virtual_crop.y as f32,
            virtual_crop.width as f32,
            virtual_crop.height as f32,
        ],
        effective: [
            effective_width as f32,
            effective_height as f32,
            opacity as f32,
            0.0,
        ],
        inverse_row0: [
            m00 as f32,
            m01 as f32,
            (anchor_x - m00 * destination_x - m01 * destination_y) as f32,
            0.0,
        ],
        inverse_row1: [
            m10 as f32,
            m11 as f32,
            (anchor_y - m10 * destination_x - m11 * destination_y) as f32,
            0.0,
        ],
        colour_row0: [
            colour.matrix[0][0] as f32,
            colour.matrix[0][1] as f32,
            colour.matrix[0][2] as f32,
            0.0,
        ],
        colour_row1: [
            colour.matrix[1][0] as f32,
            colour.matrix[1][1] as f32,
            colour.matrix[1][2] as f32,
            0.0,
        ],
        colour_row2: [
            colour.matrix[2][0] as f32,
            colour.matrix[2][1] as f32,
            colour.matrix[2][2] as f32,
            0.0,
        ],
        colour_offset: [
            colour.offset[0] as f32,
            colour.offset[1] as f32,
            colour.offset[2] as f32,
            0.0,
        ],
        solid_or_background: [0.0; 4],
    }
}

fn diagnostic(code: &str, stage: &str, error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(
        code,
        Category::Backend,
        format!("WGPU {stage} failed: {error}"),
        "",
    )
}

#[cfg(test)]
mod tests {
    use super::{WgpuBackend, compare_rgba};
    use crate::{
        animation::{Interpolation, Keyframe, Track},
        domain::{Crop, Point},
        plan::{
            CompileOptions, CompiledEffect, CompiledSizing, CompiledVisualSource, EvaluatedFrame,
            ScheduledItem, compile,
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
    fn layer_shader_parses_without_a_gpu_adapter() {
        naga::front::wgsl::parse_str(include_str!("shaders/layer.wgsl"))
            .expect("layer WGSL must parse independently of adapter availability");
    }

    #[test]
    fn rgba_comparison_reports_strict_channel_metrics() {
        let difference = compare_rgba(&[0, 2, 5, 255], &[0, 4, 4, 255], 1);
        assert_eq!(difference.maximum_absolute_channel_error, 2);
        assert_eq!(difference.differing_channels, 2);
        assert_eq!(difference.channels_exceeding_tolerance, 1);
        assert_eq!(difference.pixels_exceeding_tolerance, 1);
        assert_eq!(difference.differing_channel_percentage, 50.0);
        assert_eq!(difference.mean_absolute_channel_error, 0.75);
    }

    #[test]
    fn readback_row_alignment_matches_wgpu_copy_requirements() {
        for (width, expected) in [
            (62_u32, 256_u32),
            (64, 256),
            (66, 512),
            (318, 1280),
            (320, 1280),
            (322, 1536),
            (718, 3072),
            (720, 3072),
            (722, 3072),
            (1080, 4352),
        ] {
            assert_eq!(
                super::align_up(width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                expected
            );
        }
    }

    #[test]
    fn row_repacking_removes_padding_without_shifting_rows() {
        let row_bytes = 62 * 4;
        let padded = super::align_up(row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let mut mapped = vec![0_u8; (padded * 3) as usize];
        for row in 0..3_usize {
            mapped[row * padded as usize..row * padded as usize + row_bytes as usize]
                .fill((row + 1) as u8);
        }
        let mut contiguous = vec![0_u8; (row_bytes * 3) as usize];
        for (row, target) in contiguous.chunks_exact_mut(row_bytes as usize).enumerate() {
            let start = row * padded as usize;
            target.copy_from_slice(&mapped[start..start + row_bytes as usize]);
        }
        assert_eq!(contiguous.len(), (62 * 3 * 4) as usize);
        assert!(
            contiguous[..row_bytes as usize]
                .iter()
                .all(|byte| *byte == 1)
        );
        assert!(
            contiguous[row_bytes as usize..row_bytes as usize * 2]
                .iter()
                .all(|byte| *byte == 2)
        );
        assert!(
            contiguous[row_bytes as usize * 2..]
                .iter()
                .all(|byte| *byte == 3)
        );
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
        let mut cpu = CpuBackend::default();
        cpu.prepare(&plan, Arc::clone(&decoded))
            .expect("CPU prepares");
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
            return;
        };
        let frame = EvaluatedFrame {
            time: 0,
            background: plan.canvas.background,
            width: plan.canvas.width,
            height: plan.canvas.height,
            layers: Vec::new(),
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
        let mut cpu = CpuBackend::default();
        cpu.prepare(&plan, Arc::clone(&decoded))
            .expect("CPU prepares");
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
            ];
            let frame = crate::plan::evaluate(&plan, &[ScheduledItem(*red)], 750_000_000);
            let mut cpu = CpuBackend::default();
            cpu.prepare(&plan, Arc::clone(&decoded))
                .expect("CPU prepares");
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
        let mut cpu = CpuBackend::default();
        cpu.prepare(&plan, Arc::clone(&decoded))
            .expect("CPU prepares");
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

        for width in [62, 66, 126, 130, 318, 322, 718, 722, 1080] {
            let mut plan = canonical.clone();
            plan.canvas.width = width;
            plan.canvas.height = 18;
            let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(image_layer)], 0);
            let mut cpu = CpuBackend::default();
            cpu.prepare(&plan, Arc::clone(&decoded))
                .expect("CPU prepares");
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
            let mut cpu = CpuBackend::default();
            cpu.prepare(&plan, Arc::clone(&decoded))
                .expect("CPU prepares");
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
        let mut cpu = CpuBackend::default();
        cpu.prepare(&plan, Arc::clone(&decoded))
            .expect("CPU prepares");
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
