//! Prepared texture pipelines and bind-group layouts.

use super::parameters::PARAMETER_RECORD_BYTES;
use crate::{
    backend::RenderBackendKind,
    kernel::{EffectKernel, supports_kernel},
};

pub(super) struct GpuPipelines {
    pub(super) _layer_shader: wgpu::ShaderModule,
    pub(super) _spectrum2d_shader: wgpu::ShaderModule,
    pub(super) _composite_shader: wgpu::ShaderModule,
    pub(super) _effect_shaders: Vec<(EffectKernel, wgpu::ShaderModule)>,
    pub(super) _particle_shader: wgpu::ShaderModule,
    pub(super) _particle_resolve_shader: wgpu::ShaderModule,
    pub(super) layer: wgpu::ComputePipeline,
    pub(super) spectrum2d: wgpu::ComputePipeline,
    pub(super) composite: wgpu::ComputePipeline,
    pub(super) effects: Vec<(EffectKernel, wgpu::ComputePipeline)>,
    pub(super) particle_normal: wgpu::RenderPipeline,
    pub(super) particle_resolve: wgpu::ComputePipeline,
    pub(super) particle_resolve_bindings: wgpu::BindGroupLayout,
    pub(super) layer_bindings: wgpu::BindGroupLayout,
    pub(super) spectrum2d_bindings: wgpu::BindGroupLayout,
    pub(super) composite_bindings: wgpu::BindGroupLayout,
    pub(super) effect_bindings: wgpu::BindGroupLayout,
    pub(super) particle_bindings: wgpu::BindGroupLayout,
}

impl GpuPipelines {
    pub(super) const BASE_SHADER_MODULE_COUNT: usize = 5;
    pub(super) const BASE_PIPELINE_COUNT: usize = 5;

    pub(super) fn create(device: &wgpu::Device) -> Self {
        let layer_shader = shader(
            device,
            "vestra layer shader",
            include_str!("../shaders/layer.wgsl"),
        );
        let spectrum2d_shader = shader(
            device,
            "vestra Spectrum2D shader",
            include_str!("../shaders/spectrum2d.wgsl"),
        );
        let composite_shader = shader(
            device,
            "vestra composite shader",
            include_str!("../shaders/composite_normal.wgsl"),
        );
        let particle_shader = shader(
            device,
            "vestra particle shader",
            include_str!("../shaders/particles.wgsl"),
        );
        let particle_resolve_shader = shader(
            device,
            "vestra particle straight-alpha resolve shader",
            include_str!("../shaders/particle_resolve.wgsl"),
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
            label: Some("vestra layer texture bindings"),
            entries: &[sampled(0), storage_texture(1), uniform],
        });
        let spectrum2d_bindings =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("vestra Spectrum2D bindings"),
                entries: &[
                    storage_texture(0),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: wgpu::BufferSize::new(PARAMETER_RECORD_BYTES),
                        },
                        count: None,
                    },
                ],
            });
        let composite_bindings =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("vestra composite texture bindings"),
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
            "vestra layer pipeline",
            &layer_shader,
            &layer_bindings,
        );
        let spectrum2d = pipeline(
            device,
            "vestra Spectrum2D pipeline",
            &spectrum2d_shader,
            &spectrum2d_bindings,
        );
        let composite = pipeline(
            device,
            "vestra composite pipeline",
            &composite_shader,
            &composite_bindings,
        );
        let particle_bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vestra particle bindings"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(PARAMETER_RECORD_BYTES),
                },
                count: None,
            }],
        });
        let particle_normal = particle_pipeline(
            device,
            "vestra particle normal pipeline",
            &particle_shader,
            &particle_bindings,
        );
        let particle_resolve_bindings =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("vestra particle resolve bindings"),
                entries: &[sampled(0), storage_texture(1)],
            });
        let particle_resolve = pipeline(
            device,
            "vestra particle straight-alpha resolve pipeline",
            &particle_resolve_shader,
            &particle_resolve_bindings,
        );
        let effect_bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vestra effect texture bindings"),
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
        let effect_shaders = supported_kernels()
            .map(|kernel| (kernel, shader(device, kernel.label(), kernel.source())))
            .collect::<Vec<_>>();
        let effects = effect_shaders
            .iter()
            .map(|(kernel, shader)| {
                (
                    *kernel,
                    pipeline(device, kernel.label(), shader, &effect_bindings),
                )
            })
            .collect::<Vec<_>>();
        Self {
            _layer_shader: layer_shader,
            _spectrum2d_shader: spectrum2d_shader,
            _composite_shader: composite_shader,
            _particle_shader: particle_shader,
            _particle_resolve_shader: particle_resolve_shader,
            _effect_shaders: effect_shaders,
            layer,
            spectrum2d,
            composite,
            particle_normal,
            particle_resolve,
            particle_resolve_bindings,
            effects,
            layer_bindings,
            spectrum2d_bindings,
            composite_bindings,
            effect_bindings,
            particle_bindings,
        }
    }

    pub(super) fn effect(&self, kernel: EffectKernel) -> Option<&wgpu::ComputePipeline> {
        self.effects
            .iter()
            .find_map(|(stored, pipeline)| (*stored == kernel).then_some(pipeline))
    }

    pub(super) fn shader_module_count(&self) -> usize {
        Self::BASE_SHADER_MODULE_COUNT + self._effect_shaders.len()
    }

    pub(super) fn pipeline_count(&self) -> usize {
        Self::BASE_PIPELINE_COUNT + self.effects.len()
    }

    #[cfg(test)]
    pub(super) fn declared_supported_kernel_count() -> usize {
        supported_kernels().count()
    }
}

fn supported_kernels() -> impl Iterator<Item = EffectKernel> {
    EffectKernel::ALL
        .into_iter()
        .filter(|&kernel| supports_kernel(RenderBackendKind::Wgpu, kernel))
}

impl EffectKernel {
    const fn label(self) -> &'static str {
        match self {
            Self::ColourTransform => "vestra.effect.colour-transform",
            Self::GaussianBlur => "vestra.effect.gaussian-blur",
            Self::HighlightExtract => "vestra.effect.highlight-extract",
            Self::Composite => "vestra.effect.composite",
            Self::DirectionalBlur => "vestra.effect.directional-blur",
            Self::ZoomBlur => "vestra.effect.zoom-blur",
            Self::ChromaticAberration => "vestra.effect.chromatic-aberration",
            Self::Vignette => "vestra.effect.vignette",
            Self::ColorAdjust => "vestra.effect.color-adjust",
            Self::MotionBlur => "vestra.effect.motion-blur",
        }
    }

    pub(super) const fn source(self) -> &'static str {
        match self {
            Self::ColourTransform => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/colour_transform.wgsl")
            ),
            Self::GaussianBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/gaussian_blur.wgsl")
            ),
            Self::HighlightExtract => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/highlight_extract.wgsl")
            ),
            Self::Composite => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/composite.wgsl")
            ),
            Self::DirectionalBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/line_blur_common.wgsl"),
                include_str!("../shaders/effects/directional_blur.wgsl")
            ),
            Self::ZoomBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/zoom_blur.wgsl")
            ),
            Self::ChromaticAberration => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/chromatic_aberration.wgsl")
            ),
            Self::Vignette => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/vignette.wgsl")
            ),
            Self::ColorAdjust => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/color_adjust.wgsl")
            ),
            Self::MotionBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/line_blur_common.wgsl"),
                include_str!("../shaders/effects/motion_blur.wgsl")
            ),
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

fn particle_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader: &wgpu::ShaderModule,
    bindings: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[bindings],
        push_constant_ranges: &[],
    });
    // Normal particle fragments are straight RGB plus alpha. This pass writes
    // the premultiplied source-over representation into the dedicated
    // ParticleAccumulation texture. A resolve pass converts it before effects.
    let blend = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vertex",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 32,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 2,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 16,
                        shader_location: 3,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "fragment",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: super::texture_pool::WORKING_FORMAT,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_shader_sources_isolate_kernel_algorithms() {
        assert!(EffectKernel::GaussianBlur.source().contains("fn gaussian"));
        assert!(!EffectKernel::ColorAdjust.source().contains("fn gaussian"));
        assert!(EffectKernel::ZoomBlur.source().contains("fn zoom_blur"));
        assert!(!EffectKernel::Vignette.source().contains("fn zoom_blur"));
        assert!(
            EffectKernel::DirectionalBlur
                .source()
                .contains("fn line_blur")
        );
        assert!(!EffectKernel::Composite.source().contains("fn line_blur"));
    }

    #[test]
    fn every_declared_wgpu_kernel_has_one_pipeline_source() {
        let supported = supported_kernels().collect::<Vec<_>>();
        assert_eq!(supported.len(), EffectKernel::ALL.len());
        assert_eq!(
            supported
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            supported.len()
        );
        assert!(
            supported
                .into_iter()
                .all(|kernel| { !kernel.label().is_empty() && !kernel.source().is_empty() })
        );
    }
}
