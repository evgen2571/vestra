//! Headless WGPU resource ownership for the render backend.

#![allow(
    clippy::result_large_err,
    reason = "WGPU preparation retains structured user-facing diagnostics"
)]

use std::{sync::Arc, time::Instant};

use bytemuck::{Pod, Zeroable};
use image::RgbaImage;

use crate::{
    Category, Diagnostic,
    plan::{EvaluatedFrame, RenderPlan},
    render::{
        AdapterMetadata, RenderBackend, RenderBackendKind,
        prepared::{DecodedAssets, PreparationStats, PreparationTimings},
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
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("video-editor headless renderer"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|error| diagnostic("WGPU-DEVICE-REQUEST", "device_request", error))?;
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
        let source_bind_groups = source_textures
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
        let mut timings = decoded.timings();
        timings.texture_upload = upload_started.elapsed();
        timings.gpu_initialization = started.elapsed();
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
                ..
            } = &layer.source
            {
                let (source_width, source_height) = self.source_dimensions[*asset_index];
                let parameters = image_parameters(
                    frame,
                    source_width,
                    source_height,
                    *crop,
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
        self.queue.submit(Some(encoder.finish()));
        let slice = self.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .map_err(|error| {
                Diagnostic::error(
                    "WGPU-READBACK",
                    Category::Backend,
                    format!("readback callback failed: {error}"),
                    "",
                )
            })?
            .map_err(|error| diagnostic("WGPU-READBACK", "buffer_map", error))?;
        let mapped = slice.get_mapped_range();
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
    sizing: &crate::plan::CompiledSizing,
    transform: crate::animation::Transform2D,
    opacity: f64,
    colour: crate::plan::ColourTransform,
) -> LayerParameters {
    let cropped_width = crop.width * f64::from(source_width);
    let cropped_height = crop.height * f64::from(source_height);
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
        source: [source_width, source_height, 0, 0],
        crop: [
            crop.x as f32,
            crop.y as f32,
            crop.width as f32,
            crop.height as f32,
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
    #[test]
    fn layer_shader_parses_without_a_gpu_adapter() {
        naga::front::wgsl::parse_str(include_str!("shaders/layer.wgsl"))
            .expect("layer WGSL must parse independently of adapter availability");
    }
}
