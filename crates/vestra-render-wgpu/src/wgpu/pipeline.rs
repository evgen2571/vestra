//! Prepared texture pipelines and bind-group layouts.

use super::registry::kernel_label;
pub(super) use super::registry::kernel_source;
use super::support::supports_kernel;

use super::parameters::PARAMETER_RECORD_BYTES;
use crate::kernel::EffectKernel;

pub(super) struct GpuPipelines {
    pub(super) layer: wgpu::ComputePipeline,
    pub(super) spectrum2d: wgpu::ComputePipeline,
    pub(super) composite: wgpu::ComputePipeline,
    pub(super) mask: wgpu::ComputePipeline,
    pub(super) mask_raster: wgpu::ComputePipeline,
    pub(super) mask_coverage: wgpu::ComputePipeline,
    pub(super) mask_feather: wgpu::ComputePipeline,
    pub(super) effects: Vec<(EffectKernel, wgpu::ComputePipeline)>,
    pub(super) particle_normal: wgpu::RenderPipeline,
    pub(super) particle_resolve: wgpu::ComputePipeline,
    pub(super) particle_resolve_bindings: wgpu::BindGroupLayout,
    pub(super) layer_bindings: wgpu::BindGroupLayout,
    pub(super) spectrum2d_bindings: wgpu::BindGroupLayout,
    pub(super) composite_bindings: wgpu::BindGroupLayout,
    pub(super) mask_bindings: wgpu::BindGroupLayout,
    pub(super) mask_raster_bindings: wgpu::BindGroupLayout,
    pub(super) effect_bindings: wgpu::BindGroupLayout,
    pub(super) particle_bindings: wgpu::BindGroupLayout,
    pub(super) _layer_shader: wgpu::ShaderModule,
    pub(super) _spectrum2d_shader: wgpu::ShaderModule,
    pub(super) _composite_shader: wgpu::ShaderModule,
    pub(super) _mask_shader: wgpu::ShaderModule,
    pub(super) _mask_raster_shader: wgpu::ShaderModule,
    pub(super) _mask_coverage_shader: wgpu::ShaderModule,
    pub(super) _mask_feather_shader: wgpu::ShaderModule,
    pub(super) _effect_shaders: Vec<(EffectKernel, wgpu::ShaderModule)>,
    pub(super) _particle_shader: wgpu::ShaderModule,
    pub(super) _particle_resolve_shader: wgpu::ShaderModule,
}

impl GpuPipelines {
    pub(super) const BASE_SHADER_MODULE_COUNT: usize = 8;
    pub(super) const BASE_PIPELINE_COUNT: usize = 8;

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
        let mask_shader = shader(
            device,
            "vestra mask shader",
            include_str!("../shaders/mask.wgsl"),
        );
        let mask_coverage_shader = shader(
            device,
            "vestra mask coverage shader",
            include_str!("../shaders/mask_coverage.wgsl"),
        );
        let mask_raster_shader = shader(
            device,
            "vestra mask raster shader",
            include_str!("../shaders/mask_raster.wgsl"),
        );
        let mask_feather_shader = shader(
            device,
            "vestra mask feather shader",
            include_str!("../shaders/mask_feather.wgsl"),
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
        let mask_bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vestra mask texture bindings"),
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
        let mask_raster_bindings =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("vestra mask raster bindings"),
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
        let mask = pipeline(device, "vestra mask pipeline", &mask_shader, &mask_bindings);
        let mask_raster = pipeline(
            device,
            "vestra mask raster pipeline",
            &mask_raster_shader,
            &mask_raster_bindings,
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
                sampled(4),
                storage_texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    ..uniform
                },
            ],
        });
        let mask_coverage = pipeline(
            device,
            "vestra mask coverage pipeline",
            &mask_coverage_shader,
            &effect_bindings,
        );
        let mask_feather = pipeline(
            device,
            "vestra mask feather pipeline",
            &mask_feather_shader,
            &effect_bindings,
        );
        let effect_shaders = supported_kernels()
            .map(|kernel| {
                (
                    kernel,
                    shader(device, kernel_label(kernel), kernel_source(kernel)),
                )
            })
            .collect::<Vec<_>>();
        let effects = effect_shaders
            .iter()
            .map(|(kernel, shader)| {
                (
                    *kernel,
                    pipeline(device, kernel_label(*kernel), shader, &effect_bindings),
                )
            })
            .collect::<Vec<_>>();
        Self {
            _layer_shader: layer_shader,
            _spectrum2d_shader: spectrum2d_shader,
            _composite_shader: composite_shader,
            _mask_shader: mask_shader,
            _mask_raster_shader: mask_raster_shader,
            _mask_coverage_shader: mask_coverage_shader,
            _mask_feather_shader: mask_feather_shader,
            _particle_shader: particle_shader,
            _particle_resolve_shader: particle_resolve_shader,
            _effect_shaders: effect_shaders,
            layer,
            spectrum2d,
            composite,
            mask,
            mask_raster,
            mask_coverage,
            mask_feather,
            particle_normal,
            particle_resolve,
            particle_resolve_bindings,
            effects,
            layer_bindings,
            spectrum2d_bindings,
            composite_bindings,
            mask_bindings,
            mask_raster_bindings,
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
        .filter(|&kernel| supports_kernel(kernel))
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
            src_factor: wgpu::BlendFactor::SrcAlpha,
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
            entry_point: Some("vertex"),
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
            entry_point: Some("fragment"),
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
        entry_point: Some("compose"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_shader_labels_and_composed_bytes_match_the_registration_baseline() {
        let baseline = [
            (
                EffectKernel::HalftoneAnalyze,
                "vestra.effect.halftoneanalyze",
                4764,
                0x6a0aee56e15eff6b,
            ),
            (
                EffectKernel::Halftone,
                "vestra.effect.halftone",
                5232,
                0x02c85a992c052a48,
            ),
            (
                EffectKernel::PixelSort,
                "vestra.effect.pixelsort",
                4397,
                0xc04aca79a8dee498,
            ),
            (
                EffectKernel::Crt,
                "vestra.effect.crt",
                4349,
                0x986eaf12bd56d63e,
            ),
            (
                EffectKernel::AsciiAnalyze,
                "vestra.effect.ascii-analyze",
                6737,
                0x546d5c3659dfe960,
            ),
            (
                EffectKernel::AsciiResolve,
                "vestra.effect.ascii-resolve",
                8720,
                0xc12c0b7ca88d0c57,
            ),
            (
                EffectKernel::PaletteInputAnalyze,
                "vestra.effect.palette-input-analyze",
                3122,
                0x761c192efcd0b35b,
            ),
            (
                EffectKernel::PaletteMap,
                "vestra.effect.palette-map",
                14174,
                0xd21e997744c4d8a9,
            ),
            (
                EffectKernel::OrderedDither,
                "vestra.effect.ordered-dither",
                20352,
                0x6d56afefd456f32e,
            ),
            (
                EffectKernel::ColourTransform,
                "vestra.effect.colour-transform",
                2305,
                0x3bdeb0d582fe814e,
            ),
            (
                EffectKernel::GaussianBlur,
                "vestra.effect.gaussian-blur",
                3703,
                0x6b15caf7e36a4675,
            ),
            (
                EffectKernel::HighlightExtract,
                "vestra.effect.highlight-extract",
                2269,
                0x6221ff18ec8ac0a6,
            ),
            (
                EffectKernel::Composite,
                "vestra.effect.composite",
                2732,
                0x685332c80cd10b24,
            ),
            (
                EffectKernel::DirectionalBlur,
                "vestra.effect.directional-blur",
                2728,
                0xf1f47ae546f8b064,
            ),
            (
                EffectKernel::ZoomBlur,
                "vestra.effect.zoom-blur",
                3060,
                0x5142903d4b9df482,
            ),
            (
                EffectKernel::ChromaticAberration,
                "vestra.effect.chromatic-aberration",
                2301,
                0xfa32550b5fea5d82,
            ),
            (
                EffectKernel::Vignette,
                "vestra.effect.vignette",
                2393,
                0x26d242f21e3f09c0,
            ),
            (
                EffectKernel::ColorAdjust,
                "vestra.effect.color-adjust",
                2455,
                0xf02f6b14c5b90142,
            ),
            (
                EffectKernel::MotionBlur,
                "vestra.effect.motion-blur",
                2728,
                0xf1f47ae546f8b064,
            ),
        ];
        assert_eq!(baseline.len(), EffectKernel::ALL.len());
        for (kernel, label, length, fingerprint) in baseline {
            assert_eq!(kernel_label(kernel), label);
            let source = kernel_source(kernel).as_bytes();
            assert_eq!(source.len(), length, "{kernel:?}");
            let actual = source.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!(actual, fingerprint, "{kernel:?}");
        }
    }

    #[test]
    fn reported_base_counts_preserve_the_existing_counter_contract() {
        // Nine base resources are created; correcting their report is separate work.
        assert_eq!(GpuPipelines::BASE_SHADER_MODULE_COUNT, 8);
        assert_eq!(GpuPipelines::BASE_PIPELINE_COUNT, 8);
    }

    #[test]
    fn effect_shader_sources_isolate_kernel_algorithms() {
        assert!(kernel_source(EffectKernel::GaussianBlur).contains("fn gaussian"));
        assert!(!kernel_source(EffectKernel::ColorAdjust).contains("fn gaussian"));
        assert!(kernel_source(EffectKernel::ZoomBlur).contains("fn zoom_blur"));
        assert!(!kernel_source(EffectKernel::Vignette).contains("fn zoom_blur"));
        assert!(kernel_source(EffectKernel::DirectionalBlur).contains("fn line_blur"));
        assert!(!kernel_source(EffectKernel::Composite).contains("fn line_blur"));
    }

    #[test]
    fn every_declared_wgpu_kernel_has_one_pipeline_source() {
        let supported = supported_kernels().collect::<Vec<_>>();
        assert_eq!(
            supported
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            supported.len()
        );
        assert!(supported.into_iter().all(|kernel| {
            !kernel_label(kernel).is_empty() && !kernel_source(kernel).is_empty()
        }));
    }
}
