//! Prepared texture pipelines and bind-group layouts.

use super::parameters::PARAMETER_RECORD_BYTES;

pub(super) struct GpuPipelines {
    pub(super) _layer_shader: wgpu::ShaderModule,
    pub(super) _composite_shader: wgpu::ShaderModule,
    pub(super) _effect_shader: wgpu::ShaderModule,
    pub(super) layer: wgpu::ComputePipeline,
    pub(super) composite: wgpu::ComputePipeline,
    pub(super) effect: wgpu::ComputePipeline,
    pub(super) layer_bindings: wgpu::BindGroupLayout,
    pub(super) composite_bindings: wgpu::BindGroupLayout,
    pub(super) effect_bindings: wgpu::BindGroupLayout,
    pub(super) parameters: wgpu::Buffer,
}

impl GpuPipelines {
    pub(super) fn create(device: &wgpu::Device, parameter_buffer_bytes: u64) -> Self {
        let layer_shader = shader(
            device,
            "video-editor layer shader",
            include_str!("../shaders/layer.wgsl"),
        );
        let composite_shader = shader(
            device,
            "video-editor composite shader",
            include_str!("../shaders/composite_normal.wgsl"),
        );
        let effect_shader = shader(
            device,
            "video-editor effect shader",
            include_str!("../shaders/effects.wgsl"),
        );
        let uniform = wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(PARAMETER_RECORD_BYTES),
            },
            count: None,
        };
        let layer_bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video-editor layer texture bindings"),
            entries: &[sampled(0), storage_texture(1), uniform],
        });
        let composite_bindings =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("video-editor composite texture bindings"),
                entries: &[
                    sampled(0),
                    sampled(1),
                    storage_texture(2),
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        ..uniform
                    },
                ],
            });
        let layer = pipeline(
            device,
            "video-editor layer pipeline",
            &layer_shader,
            &layer_bindings,
        );
        let composite = pipeline(
            device,
            "video-editor composite pipeline",
            &composite_shader,
            &composite_bindings,
        );
        let effect_bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video-editor effect texture bindings"),
            entries: &[
                sampled(0),
                sampled(1),
                storage_texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    ..uniform
                },
            ],
        });
        let effect = pipeline(
            device,
            "video-editor effect pipeline",
            &effect_shader,
            &effect_bindings,
        );
        let parameters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-editor frame parameters"),
            size: parameter_buffer_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            _layer_shader: layer_shader,
            _composite_shader: composite_shader,
            _effect_shader: effect_shader,
            layer,
            composite,
            effect,
            layer_bindings,
            composite_bindings,
            effect_bindings,
            parameters,
        }
    }
}

fn shader(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}
fn sampled(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            multisampled: false,
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
        },
        count: None,
    }
}
fn storage_texture(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}
fn pipeline(
    device: &wgpu::Device,
    label: &str,
    shader: &wgpu::ShaderModule,
    bindings: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[bindings],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        module: shader,
        entry_point: "compose",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}
