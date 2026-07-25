//! Shader, bindings, compute pipeline, and shared layer uniform buffer.

use super::parameters::LayerParameters;

pub(super) struct LayerPipeline {
    pub(super) _shader: wgpu::ShaderModule,
    pub(super) compute: wgpu::ComputePipeline,
    pub(super) bindings: wgpu::BindGroupLayout,
    pub(super) parameters: wgpu::Buffer,
}

impl LayerPipeline {
    pub(super) fn create(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video-editor layer compute shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/layer.wgsl").into()),
        });
        let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<LayerParameters>() as u64,
                        ),
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video-editor layer pipeline layout"),
            bind_group_layouts: &[&bindings],
            push_constant_ranges: &[],
        });
        let compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("video-editor layer pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: "compose",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let parameters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-editor layer parameters"),
            size: std::mem::size_of::<LayerParameters>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            _shader: shader,
            compute,
            bindings,
            parameters,
        }
    }
}
